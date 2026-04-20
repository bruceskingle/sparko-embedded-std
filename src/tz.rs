pub const TIMEZONE_LEN: usize = 64;
pub const TZ_EUROPE_LONDON: &str = "Europe/London";
pub const TZ_EUROPE_BERLIN: &str = "Europe/Berlin";
pub const TZ_EUROPE_PARIS: &str = "Europe/Paris";
pub const TZ_EUROPE_MADRID: &str = "Europe/Madrid";
pub const TZ_EUROPE_ROME: &str = "Europe/Rome";

pub const TZ_AMERICA_NEW_YORK: &str = "America/New_York";
pub const TZ_AMERICA_CHICAGO: &str = "America/Chicago";
pub const TZ_AMERICA_DENVER: &str = "America/Denver";
pub const TZ_AMERICA_LOS_ANGELES: &str = "America/Los_Angeles";

pub const TZ_AMERICA_TORONTO: &str = "America/Toronto";
pub const TZ_AMERICA_VANCOUVER: &str = "America/Vancouver";

pub const TZ_AUSTRALIA_SYDNEY: &str = "Australia/Sydney";
pub const TZ_AUSTRALIA_MELBOURNE: &str = "Australia/Melbourne";
pub const TZ_AUSTRALIA_BRISBANE: &str = "Australia/Brisbane";

pub const TZ_PACIFIC_AUCKLAND: &str = "Pacific/Auckland";

pub const TZ_ASIA_TOKYO: &str = "Asia/Tokyo";
pub const TZ_ASIA_SHANGHAI: &str = "Asia/Shanghai";
pub const TZ_ASIA_KOLKATA: &str = "Asia/Kolkata";
pub const TZ_ASIA_SINGAPORE: &str = "Asia/Singapore";
pub const TZ_ASIA_DUBAI: &str = "Asia/Dubai";

pub const TZ_AMERICA_SAO_PAULO: &str = "America/Sao_Paulo";

pub const TZ_UTC: &str = "UTC";
pub const TZ_UTC_ALT: &str = "Etc/UTC";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZone {
    EuropeLondon,
    EuropeBerlin,
    EuropeParis,
    EuropeMadrid,
    EuropeRome,

    AmericaNewYork,
    AmericaChicago,
    AmericaDenver,
    AmericaLosAngeles,

    AmericaToronto,
    AmericaVancouver,

    AustraliaSydney,
    AustraliaMelbourne,
    AustraliaBrisbane,

    PacificAuckland,

    AsiaTokyo,
    AsiaShanghai,
    AsiaKolkata,
    AsiaSingapore,
    AsiaDubai,

    AmericaSaoPaulo,

    Utc,
}


pub const ALL: &'static [TimeZone] = &[
    // Europe
    TimeZone::EuropeLondon,
    TimeZone::EuropeBerlin,
    TimeZone::EuropeParis,
    TimeZone::EuropeMadrid,
    TimeZone::EuropeRome,

    // USA
    TimeZone::AmericaNewYork,
    TimeZone::AmericaChicago,
    TimeZone::AmericaDenver,
    TimeZone::AmericaLosAngeles,

    // Canada
    TimeZone::AmericaToronto,
    TimeZone::AmericaVancouver,

    // Australia
    TimeZone::AustraliaSydney,
    TimeZone::AustraliaMelbourne,
    TimeZone::AustraliaBrisbane,

    // New Zealand
    TimeZone::PacificAuckland,

    // Asia
    TimeZone::AsiaTokyo,
    TimeZone::AsiaShanghai,
    TimeZone::AsiaKolkata,
    TimeZone::AsiaSingapore,
    TimeZone::AsiaDubai,

    // South America
    TimeZone::AmericaSaoPaulo,

    // UTC
    TimeZone::Utc,
];

impl /*ConfigSerializable for*/ TimeZone {
    pub fn iter() -> impl Iterator<Item = &'static TimeZone> {
        ALL.iter()
    }

    pub fn to_posix_tz(&self) -> &'static str {
        match self {
            // 🇬🇧
            TimeZone::EuropeLondon => "GMT0BST,M3.5.0/1,M10.5.0/2",

            // 🇪🇺
            TimeZone::EuropeBerlin
            | TimeZone::EuropeParis
            | TimeZone::EuropeMadrid
            | TimeZone::EuropeRome => "CET-1CEST,M3.5.0/2,M10.5.0/3",

            // 🇺🇸
            TimeZone::AmericaNewYork
            | TimeZone::AmericaToronto => "EST5EDT,M3.2.0/2,M11.1.0/2",

            TimeZone::AmericaChicago => "CST6CDT,M3.2.0/2,M11.1.0/2",
            TimeZone::AmericaDenver  => "MST7MDT,M3.2.0/2,M11.1.0/2",

            TimeZone::AmericaLosAngeles
            | TimeZone::AmericaVancouver => "PST8PDT,M3.2.0/2,M11.1.0/2",

            // 🇦🇺
            TimeZone::AustraliaSydney
            | TimeZone::AustraliaMelbourne => "AEST-10AEDT,M10.1.0/2,M4.1.0/3",

            TimeZone::AustraliaBrisbane => "AEST-10",

            // 🇳🇿
            TimeZone::PacificAuckland => "NZST-12NZDT,M9.5.0/2,M4.1.0/3",

            // 🇯🇵
            TimeZone::AsiaTokyo => "JST-9",

            // 🇨🇳
            TimeZone::AsiaShanghai => "CST-8",

            // 🇮🇳
            TimeZone::AsiaKolkata => "IST-5:30",

            // 🇸🇬
            TimeZone::AsiaSingapore => "SGT-8",

            // 🇦🇪
            TimeZone::AsiaDubai => "GST-4",

            // 🇧🇷
            TimeZone::AmericaSaoPaulo => "BRT3",

            // 🌍
            TimeZone::Utc => "UTC0",
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            TimeZone::EuropeLondon => TZ_EUROPE_LONDON,
            TimeZone::EuropeBerlin => TZ_EUROPE_BERLIN,
            TimeZone::EuropeParis => TZ_EUROPE_PARIS,
            TimeZone::EuropeMadrid => TZ_EUROPE_MADRID,
            TimeZone::EuropeRome => TZ_EUROPE_ROME,

            TimeZone::AmericaNewYork => TZ_AMERICA_NEW_YORK,
            TimeZone::AmericaChicago => TZ_AMERICA_CHICAGO,
            TimeZone::AmericaDenver => TZ_AMERICA_DENVER,
            TimeZone::AmericaLosAngeles => TZ_AMERICA_LOS_ANGELES,

            TimeZone::AmericaToronto => TZ_AMERICA_TORONTO,
            TimeZone::AmericaVancouver => TZ_AMERICA_VANCOUVER,

            TimeZone::AustraliaSydney => TZ_AUSTRALIA_SYDNEY,
            TimeZone::AustraliaMelbourne => TZ_AUSTRALIA_MELBOURNE,
            TimeZone::AustraliaBrisbane => TZ_AUSTRALIA_BRISBANE,

            TimeZone::PacificAuckland => TZ_PACIFIC_AUCKLAND,

            TimeZone::AsiaTokyo => TZ_ASIA_TOKYO,
            TimeZone::AsiaShanghai => TZ_ASIA_SHANGHAI,
            TimeZone::AsiaKolkata => TZ_ASIA_KOLKATA,
            TimeZone::AsiaSingapore => TZ_ASIA_SINGAPORE,
            TimeZone::AsiaDubai => TZ_ASIA_DUBAI,

            TimeZone::AmericaSaoPaulo => TZ_AMERICA_SAO_PAULO,

            TimeZone::Utc => TZ_UTC,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            TZ_EUROPE_LONDON => Some(Self::EuropeLondon),
            TZ_EUROPE_BERLIN => Some(Self::EuropeBerlin),
            TZ_EUROPE_PARIS => Some(Self::EuropeParis),
            TZ_EUROPE_MADRID => Some(Self::EuropeMadrid),
            TZ_EUROPE_ROME => Some(Self::EuropeRome),

            TZ_AMERICA_NEW_YORK => Some(Self::AmericaNewYork),
            TZ_AMERICA_CHICAGO => Some(Self::AmericaChicago),
            TZ_AMERICA_DENVER => Some(Self::AmericaDenver),
            TZ_AMERICA_LOS_ANGELES => Some(Self::AmericaLosAngeles),

            TZ_AMERICA_TORONTO => Some(Self::AmericaToronto),
            TZ_AMERICA_VANCOUVER => Some(Self::AmericaVancouver),

            TZ_AUSTRALIA_SYDNEY => Some(Self::AustraliaSydney),
            TZ_AUSTRALIA_MELBOURNE => Some(Self::AustraliaMelbourne),
            TZ_AUSTRALIA_BRISBANE => Some(Self::AustraliaBrisbane),

            TZ_PACIFIC_AUCKLAND => Some(Self::PacificAuckland),

            TZ_ASIA_TOKYO => Some(Self::AsiaTokyo),
            TZ_ASIA_SHANGHAI => Some(Self::AsiaShanghai),
            TZ_ASIA_KOLKATA => Some(Self::AsiaKolkata),
            TZ_ASIA_SINGAPORE => Some(Self::AsiaSingapore),
            TZ_ASIA_DUBAI => Some(Self::AsiaDubai),

            TZ_AMERICA_SAO_PAULO => Some(Self::AmericaSaoPaulo),

            TZ_UTC | TZ_UTC_ALT => Some(Self::Utc),

            _ => None,
        }
    }
}



