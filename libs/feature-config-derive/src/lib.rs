use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, Lit, Meta, NestedMeta,
    PathArguments, Type,
};

/// Derive macro for `FeatureConfig`.
///
/// The `TypedValue` variant is inferred from the field's Rust type:
///
/// | Field type          | `TypedValue` variant        |
/// |---------------------|-----------------------------|
/// | `i32`               | `TypedValue::Int32`         |
/// | `i64`               | `TypedValue::Int64`         |
/// | `bool`              | `TypedValue::Bool`          |
/// | `TimeZone`          | `TypedValue::TimeZone`      |
/// | `Cron`              | `TypedValue::Cron`          |
/// | `RGB8`              | `TypedValue::Color`         |
/// | `String`            | **requires** `#[config(len = N)]` |
/// | `Option<T>`         | same as `T`, but `required = false` |
///
/// `String` fields must carry `#[config(len = N)]` because the length cap
/// is not visible in the type system and must be supplied explicitly.
///
/// # Example
///
/// ```rust
/// #[derive(FeatureConfig)]
/// pub struct AnalogClockConfig {
///     pub clock_color: RGB8,
///     pub bg_color: Option<RGB8>,
/// }
///
/// #[derive(FeatureConfig)]
/// pub struct WidgetConfig {
///     #[config(len = 64)]
///     pub title: String,
///
///     #[config(len = 32)]
///     pub subtitle: Option<String>,
///
///     pub brightness: i32,
///     pub show_seconds: bool,
///     pub tz: Option<TimeZone>,
///     pub schedule: Option<Cron>,
///     pub fg_color: RGB8,
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

/// The only attribute we read is the optional `#[config(len = N)]` on `String` fields.
fn parse_string_len_attr(field: &syn::Field) -> syn::Result<Option<usize>> {
    for attr in &field.attrs {
        if !attr.path.is_ident("config") {
            continue;
        }
        let meta = attr.parse_meta()?;
        if let Meta::List(list) = meta {
            for nested in &list.nested {
                if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                    if nv.path.is_ident("len") {
                        if let Lit::Int(n) = &nv.lit {
                            return Ok(Some(n.base10_parse()?));
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.lit,
                                "`len` value must be an integer literal",
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

// ── type classification ──────────────────────────────────────────────────────

/// Every `TypedValue` variant except `String` (which needs a runtime length).
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

/// Match the last path segment's ident to a `KnownType`, or return `None` for
/// `String` (handled separately) and an error for anything unrecognised.
fn classify_type(ty: &Type, field: &syn::Field) -> syn::Result<Option<KnownType>> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "i32" => Ok(Some(KnownType::Int32)),
                "i64" => Ok(Some(KnownType::Int64)),
                "bool" => Ok(Some(KnownType::Bool)),
                "TimeZone" => Ok(Some(KnownType::TimeZone)),
                "Cron" => Ok(Some(KnownType::Cron)),
                "RGB8" => Ok(Some(KnownType::Color)),
                "String" => Ok(None), // caller must check for #[config(len = N)]
                other => Err(syn::Error::new_spanned(
                    field,
                    format!(
                        "cannot map `{other}` to a TypedValue variant; \
                         supported types: i32, i64, bool, TimeZone, Cron, RGB8, String"
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

// ── code-generation helpers ──────────────────────────────────────────────────

/// Whether a value of this type should be copied or cloned out of the match arm.
fn is_copy(kt: KnownType) -> bool {
    matches!(
        kt,
        KnownType::Int32 | KnownType::Int64 | KnownType::Bool | KnownType::Color
    )
}

/// `TypedValue::Xxx(Some(val))` pattern + extraction expression for a known type.
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

// ── per-field emission ───────────────────────────────────────────────────────

/// Describes how a single field maps to a `TypedValue`.
enum FieldKind {
    /// `String` field with a compile-time length cap.
    Str { len: usize, optional: bool },
    /// Any other supported type.
    Known { kt: KnownType, optional: bool },
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
        Some(kt) => {
            // len = N is only meaningful on String fields; reject it on everything else.
            if parse_string_len_attr(field)?.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "`#[config(len = N)]` is only valid on `String` fields",
                ));
            }
            Ok(FieldKind::Known { kt, optional })
        }
        None => {
            // String — must have #[config(len = N)]
            match parse_string_len_attr(field)? {
                Some(len) => Ok(FieldKind::Str { len, optional }),
                None => Err(syn::Error::new_spanned(
                    field,
                    "`String` fields require `#[config(len = N)]` to specify the maximum length",
                )),
            }
        }
    }
}

fn emit_from_extraction(field_ident: &syn::Ident, kind: &FieldKind, key: &str) -> TokenStream2 {
    let field_str = field_ident.to_string();

    match kind {
        FieldKind::Str {
            len,
            optional: false,
        } => quote! {
            let #field_ident = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) => val.clone(),
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => anyhow::bail!("Missing required config value: {}", #key),
            };
            let _ = #len; // len is validated at save-time, not read-time
        },

        FieldKind::Str {
            len,
            optional: true,
        } => quote! {
            let #field_ident = match spec.map.get(#key) {
                Some(ConfigSpecValue { value: TypedValue::String(_, Some(val)), .. }) => Some(val.clone()),
                Some(ConfigSpecValue { value, .. }) if value.is_none() => None,
                Some(other) => anyhow::bail!(
                    "Invalid type for {}: expected String, got {:?}",
                    #field_str, other.value,
                ),
                None => None,
            };
            let _ = #len;
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
                        #field_str,
                        other.value.to_none(),
                        other.value,
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
                        #field_str,
                        other.value.to_none(),
                        other.value,
                    ),
                    None => None,
                };
            }
        }
    }
}

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
        let key = field_ident.to_string();
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
