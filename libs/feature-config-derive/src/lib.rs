use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, Fields, GenericArgument, PathArguments, Type,
};

/// Derive macro for `FeatureConfig`.
///
/// The `TypedValue` variant is inferred from the field's Rust type:
///
/// | Field type              | `TypedValue` variant             | Attribute required    |
/// |-------------------------|----------------------------------|-----------------------|
/// | `i32`                   | `TypedValue::Int32`              | —                     |
/// | `i64`                   | `TypedValue::Int64`              | —                     |
/// | `bool`                  | `TypedValue::Bool`               | —                     |
/// | `TimeZone`              | `TypedValue::TimeZone`           | —                     |
/// | `Cron`                  | `TypedValue::Cron`               | —                     |
/// | `RGB8`                  | `TypedValue::Color`              | —                     |
/// | `String`                | `TypedValue::String(N, ...)`     | `#[config(len = N)]`  |
/// | `heapless::String<N>`   | `TypedValue::String(N, ...)`     | —                     |
/// | `Option<T>`             | same as `T`, `required = false`  | as above if needed    |
///
/// The `N` in `heapless::String<N>` may be a literal (`64`) or a named
/// constant (`PASSWORD_LEN`). Field names must be 15 characters or fewer.
///
/// # Example
///
/// ```rust
/// const SSID_LEN: usize = 32;
///
/// #[derive(FeatureConfig)]
/// pub struct CoreConfig {
///     #[config(len = 32)]
///     pub ssid: String,
///     pub wifi_password: heapless::String<64>,
///     pub ap_password: heapless::String<SSID_LEN>,
///     pub time_zone: TimeZone,
///     pub clock_color: RGB8,
///     pub bg_color: Option<RGB8>,
/// }
/// ```
#[proc_macro_derive(FeatureConfig, attributes(config))]
pub fn derive_feature_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_feature_config(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

// ── attribute parsing ────────────────────────────────────────────────────────

/// Reads `#[config(len = N)]` from a field, returning the value as an `Expr`
/// so it can be a literal or a named constant, consistent with heapless fields.
fn parse_string_len_attr(field: &syn::Field) -> syn::Result<Option<Expr>> {
    for attr in &field.attrs {
        if !attr.path().is_ident("config") {
            continue;
        }
        let mut len_expr: Option<Expr> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("len") {
                let value = meta.value()?; // consumes the `=`
                len_expr = Some(value.parse()?);
                Ok(())
            } else {
                Err(meta.error("unknown config attribute; expected `len = N`"))
            }
        })?;
        if len_expr.is_some() {
            return Ok(len_expr);
        }
    }
    Ok(None)
}

// ── type classification ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum KnownType {
    Int32,
    Int64,
    Bool,
    TimeZone,
    Cron,
    Color,
}

/// Returns `Some(inner_ty)` when `ty` is `Option<inner_ty>`, otherwise `None`.
fn unwrap_option(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if seg.ident != "Option" {
            return None;
        }
        if let PathArguments::AngleBracketed(ab) = &seg.arguments {
            if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                return Some(inner);
            }
        }
    }
    None
}

/// Result of classifying an inner (non-Option) field type.
/// `HeaplessStr` carries the const-generic expression verbatim so it works
/// with both literals (`64`) and named constants (`PASSWORD_LEN`).
enum TypeClass {
    Known(KnownType),
    /// The `Expr` is the const generic N from `heapless::String<N>`.
    HeaplessStr(Expr),
    /// Plain `String` — caller must supply a `#[config(len = N)]` attribute.
    StdStr,
}

fn classify_type(ty: &Type, field: &syn::Field) -> syn::Result<TypeClass> {
    if let Type::Path(tp) = ty {
        let segments: Vec<_> = tp.path.segments.iter().collect();

        // heapless::String<N> — match on last two segments so a leading `::`
        // or extra qualifier prefix doesn't break the check.
        let is_heapless_string = segments.len() >= 2
            && segments[segments.len() - 2].ident == "heapless"
            && segments[segments.len() - 1].ident == "String";

        if is_heapless_string {
            let last = segments.last().unwrap();
            if let PathArguments::AngleBracketed(ab) = &last.arguments {
                let len_expr: Option<Expr> = match ab.args.first() {
                    Some(GenericArgument::Type(Type::Path(inner_tp))) => {
                        Some(Expr::Path(syn::ExprPath {
                            attrs: vec![],
                            qself: inner_tp.qself.clone(),
                            path: inner_tp.path.clone(),
                        }))
                    }
                    Some(GenericArgument::Const(expr)) => Some(expr.clone()),
                    _ => None,
                };
                if let Some(expr) = len_expr {
                    return Ok(TypeClass::HeaplessStr(expr));
                }
            }
            return Err(syn::Error::new_spanned(
                field,
                "`heapless::String` fields must carry a const generic length: \
                 `heapless::String<N>` or `heapless::String<MY_CONST>`",
            ));
        }

        // single-segment types
        if let Some(seg) = tp.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "i32" => Ok(TypeClass::Known(KnownType::Int32)),
                "i64" => Ok(TypeClass::Known(KnownType::Int64)),
                "bool" => Ok(TypeClass::Known(KnownType::Bool)),
                "TimeZone" => Ok(TypeClass::Known(KnownType::TimeZone)),
                "Cron" => Ok(TypeClass::Known(KnownType::Cron)),
                "RGB8" => Ok(TypeClass::Known(KnownType::Color)),
                "String" => Ok(TypeClass::StdStr),
                other => Err(syn::Error::new_spanned(
                    field,
                    format!(
                        "cannot map `{other}` to a TypedValue variant; \
                         supported types: i32, i64, bool, TimeZone, Cron, RGB8, \
                         String (needs #[config(len = N)]), heapless::String<N>"
                    ),
                )),
            };
        }
    }
    Err(syn::Error::new_spanned(
        field,
        "unsupported field type for FeatureConfig derive",
    ))
}

// ── per-field kind ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum StringKind {
    Std,
    Heapless,
}

/// The length expression for a string field — either from a `#[config(len = N)]`
/// attribute (plain `String`) or from the const generic (heapless). Both cases
/// carry an `Expr` so they emit identically in generated code.
enum FieldKind {
    Str {
        len: Expr,
        kind: StringKind,
        optional: bool,
    },
    Known {
        kt: KnownType,
        optional: bool,
    },
}

fn field_kind(field: &syn::Field) -> syn::Result<FieldKind> {
    let field_ident = field.ident.as_ref().unwrap();
    if field_ident.to_string().len() > 15 {
        return Err(syn::Error::new_spanned(
            field_ident,
            format!(
                "config field name `{}` is too long: max length is 15 characters",
                field_ident
            ),
        ));
    }

    let (inner_ty, optional) = match unwrap_option(&field.ty) {
        Some(inner) => (inner, true),
        None => (&field.ty, false),
    };

    match classify_type(inner_ty, field)? {
        TypeClass::HeaplessStr(len) => {
            if parse_string_len_attr(field)?.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "`#[config(len = N)]` is redundant on `heapless::String<N>`: \
                     the length is already in the type",
                ));
            }
            Ok(FieldKind::Str {
                len,
                kind: StringKind::Heapless,
                optional,
            })
        }
        TypeClass::StdStr => match parse_string_len_attr(field)? {
            Some(len) => Ok(FieldKind::Str {
                len,
                kind: StringKind::Std,
                optional,
            }),
            None => Err(syn::Error::new_spanned(
                field,
                "`String` fields require `#[config(len = N)]` to specify the maximum length",
            )),
        },
        TypeClass::Known(kt) => {
            if parse_string_len_attr(field)?.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "`#[config(len = N)]` is only valid on `String` or `heapless::String<N>` fields",
                ));
            }
            Ok(FieldKind::Known { kt, optional })
        }
    }
}

// ── code-generation helpers ──────────────────────────────────────────────────

fn is_copy(kt: KnownType) -> bool {
    matches!(
        kt,
        KnownType::Int32 | KnownType::Int64 | KnownType::Bool | KnownType::Color
    )
}

fn match_arm_known(kt: KnownType) -> (TokenStream2, TokenStream2) {
    if is_copy(kt) {
        let pat = match kt {
            KnownType::Int32 => quote! { TypedValue::Int32(Some(val)) },
            KnownType::Int64 => quote! { TypedValue::Int64(Some(val)) },
            KnownType::Bool => quote! { TypedValue::Bool(Some(val))  },
            KnownType::Color => quote! { TypedValue::Color(Some(val)) },
            _ => unreachable!(),
        };
        (pat, quote! { *val })
    } else {
        let pat = match kt {
            KnownType::TimeZone => quote! { TypedValue::TimeZone(Some(val)) },
            KnownType::Cron => quote! { TypedValue::Cron(Some(val))     },
            _ => unreachable!(),
        };
        (pat, quote! { val.clone() })
    }
}

// ── from_config_spec emission ────────────────────────────────────────────────

fn emit_from_extraction(field_ident: &syn::Ident, kind: &FieldKind, key: &str) -> TokenStream2 {
    let field_str = field_ident.to_string();

    match kind {
        FieldKind::Str {
            kind: StringKind::Std,
            optional: false,
            ..
        } => quote! {
            let #field_ident: String = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) => val.clone(),
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => anyhow::bail!("Missing required config value: {}", #key),
            };
        },

        FieldKind::Str {
            kind: StringKind::Std,
            optional: true,
            ..
        } => quote! {
            let #field_ident: Option<String> = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) => Some(val.clone()),
                Some(ConfigSpecValue { value, .. }) if value.is_none() => None,
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => None,
            };
        },

        FieldKind::Str {
            len,
            kind: StringKind::Heapless,
            optional: false,
        } => quote! {
            let #field_ident: heapless::String<#len> = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) =>
                    heapless::String::try_from(val.as_str())
                        .map_err(|_| anyhow::anyhow!(
                            "value too long for `{}`: max length is {}",
                            #field_str, #len,
                        ))?,
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => anyhow::bail!("Missing required config value: {}", #key),
            };
        },

        FieldKind::Str {
            len,
            kind: StringKind::Heapless,
            optional: true,
        } => quote! {
            let #field_ident: Option<heapless::String<#len>> = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) =>
                    Some(heapless::String::try_from(val.as_str())
                        .map_err(|_| anyhow::anyhow!(
                            "value too long for `{}`: max length is {}",
                            #field_str, #len,
                        ))?),
                Some(ConfigSpecValue { value, .. }) if value.is_none() => None,
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => None,
            };
        },

        FieldKind::Known {
            kt,
            optional: false,
        } => {
            let (pattern, extraction) = match_arm_known(*kt);
            quote! {
                let #field_ident = match spec.map.get(#key) {
                    Some(ConfigSpecValue { value: #pattern, .. }) => #extraction,
                    Some(other) => anyhow::bail!(
                        "Invalid type for {}: expected {:?}, got {:?}",
                        #field_str, other.value.to_none(), other.value,
                    ),
                    None => anyhow::bail!("Missing required config value: {}", #key),
                };
            }
        }

        FieldKind::Known { kt, optional: true } => {
            let (pattern, extraction) = match_arm_known(*kt);
            quote! {
                let #field_ident = match spec.map.get(#key) {
                    Some(ConfigSpecValue { value: #pattern, .. }) => Some(#extraction),
                    Some(ConfigSpecValue { value, .. }) if value.is_none() => None,
                    Some(other) => anyhow::bail!(
                        "Invalid type for {}: expected {:?}, got {:?}",
                        #field_str, other.value.to_none(), other.value,
                    ),
                    None => None,
                };
            }
        }
    }
}

// ── to_config_spec emission ──────────────────────────────────────────────────

/// Emits the `.with(...)` call for one field.
/// All values are `None` — `to_config_spec` describes the schema, not an instance.
/// For string fields the length expression is emitted as-is, so both literals
/// and named constants work correctly.
fn emit_to_with(kind: &FieldKind, key: &str) -> TokenStream2 {
    let typed_value_none = match kind {
        FieldKind::Str { len, .. } => quote! { TypedValue::String(#len, None) },
        FieldKind::Known {
            kt: KnownType::Int32,
            ..
        } => quote! { TypedValue::Int32(None)    },
        FieldKind::Known {
            kt: KnownType::Int64,
            ..
        } => quote! { TypedValue::Int64(None)    },
        FieldKind::Known {
            kt: KnownType::Bool,
            ..
        } => quote! { TypedValue::Bool(None)     },
        FieldKind::Known {
            kt: KnownType::TimeZone,
            ..
        } => quote! { TypedValue::TimeZone(None) },
        FieldKind::Known {
            kt: KnownType::Cron,
            ..
        } => quote! { TypedValue::Cron(None)     },
        FieldKind::Known {
            kt: KnownType::Color,
            ..
        } => quote! { TypedValue::Color(None)    },
    };
    let required = matches!(
        kind,
        FieldKind::Str {
            optional: false,
            ..
        } | FieldKind::Known {
            optional: false,
            ..
        }
    );
    quote! {
        .with(
            #key.to_string(),
            ConfigSpecValue::new(#typed_value_none, #required),
        )?
    }
}

// ── main codegen ─────────────────────────────────────────────────────────────

fn impl_feature_config(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "FeatureConfig can only be derived for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "FeatureConfig can only be derived for structs",
            ))
        }
    };

    let mut from_extractions = Vec::new();
    let mut from_field_inits = Vec::new();
    let mut to_withs = Vec::new();

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let key = field_ident.to_string().to_uppercase();
        let kind = field_kind(field)?;

        from_extractions.push(emit_from_extraction(field_ident, &kind, &key));
        to_withs.push(emit_to_with(&kind, &key));
        from_field_inits.push(quote! { #field_ident, });
    }

    Ok(quote! {
        impl FeatureConfig for #name {
            fn from_config_spec(spec: &ConfigSpec) -> anyhow::Result<Self> {
                #(#from_extractions)*
                Ok(#name { #(#from_field_inits)* })
            }

            fn to_config_spec() -> anyhow::Result<ConfigSpec> {
                Ok(ConfigSpec::builder()
                    #(#to_withs)*
                    .build())
            }
        }
    })
}
