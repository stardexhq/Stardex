//! Amount, asset and address helpers for creating invoices.

use std::str::FromStr;

use stellar_xdr::{AccountId, MuxedAccountMed25519, PublicKey, Uint256};

/// Classic Stellar assets (XLM and issued assets) have 7 decimal places.
pub const CLASSIC_DECIMALS: u32 = 7;

/// Parse a decimal amount like `"12.5"` into raw units of a classic asset.
/// Rejects negative or zero amounts and more than 7 decimal places.
pub fn parse_amount(text: &str) -> Result<i128, String> {
    let text = text.trim();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (whole, frac, frac_ok) = match text.split_once('.') {
        Some((whole, frac)) => (whole, frac, digits(frac)),
        None => (text, "", true),
    };
    if !digits(whole) || !frac_ok {
        return Err(format!("{text:?} is not a valid amount"));
    }
    if frac.len() > CLASSIC_DECIMALS as usize {
        return Err(format!(
            "{text:?} has more than {CLASSIC_DECIMALS} decimal places"
        ));
    }
    let scale = 10i128.pow(CLASSIC_DECIMALS);
    let whole: i128 = whole
        .parse()
        .map_err(|_| format!("{text:?} is too large"))?;
    let frac: i128 = format!("{frac:0<7}").parse().unwrap_or(0);
    let units = whole
        .checked_mul(scale)
        .and_then(|w| w.checked_add(frac))
        .ok_or_else(|| format!("{text:?} is too large"))?;
    if units == 0 {
        return Err("amount must be greater than zero".into());
    }
    Ok(units)
}

/// Format raw units of a classic asset with 7 decimal places.
pub fn format_amount(units: i128) -> String {
    let sign = if units < 0 { "-" } else { "" };
    let abs = units.unsigned_abs();
    let scale = 10u128.pow(CLASSIC_DECIMALS);
    format!("{sign}{}.{:07}", abs / scale, abs % scale)
}

/// Accepts `native` or a SEP-11 `CODE:ISSUER` with a 1 to 12 character
/// alphanumeric code and a valid `G...` issuer.
pub fn validate_asset(asset: &str) -> Result<(), String> {
    if asset == "native" {
        return Ok(());
    }
    let invalid = || format!("{asset:?} is not \"native\" or CODE:ISSUER");
    let (code, issuer) = asset.split_once(':').ok_or_else(invalid)?;
    let code_ok = (1..=12).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphanumeric());
    if !code_ok || AccountId::from_str(issuer).is_err() {
        return Err(invalid());
    }
    Ok(())
}

/// The muxed `M...` address for `account` with `id`. Paying it lands in
/// `account` with `id` attached, the same as paying `account` with that
/// number as a MEMO_ID.
pub fn muxed_address(account: &str, id: u64) -> Result<String, String> {
    let AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(key))) = AccountId::from_str(account)
        .map_err(|_| format!("{account} is not a Stellar account address (G...)"))?;
    Ok(MuxedAccountMed25519 {
        id,
        ed25519: Uint256(key),
    }
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_amounts_into_units() {
        assert_eq!(parse_amount("5"), Ok(50_000_000));
        assert_eq!(parse_amount("2.5"), Ok(25_000_000));
        assert_eq!(parse_amount("0.0000001"), Ok(1));
        assert_eq!(parse_amount(" 1250.75 "), Ok(12_507_500_000));
    }

    #[test]
    fn rejects_bad_amounts() {
        for bad in [
            "",
            "0",
            "0.0",
            "-5",
            "1.",
            ".5",
            "1.2.3",
            "abc",
            "1.00000001",
            "1e3",
        ] {
            assert!(parse_amount(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn formats_units_with_seven_decimals() {
        assert_eq!(format_amount(50_000_000), "5.0000000");
        assert_eq!(format_amount(1), "0.0000001");
        assert_eq!(
            format_amount(parse_amount("1250.75").unwrap()),
            "1250.7500000"
        );
    }

    #[test]
    fn validates_assets() {
        assert!(validate_asset("native").is_ok());
        assert!(
            validate_asset("USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN").is_ok()
        );
        for bad in [
            "",
            "XLM",
            "USDC",
            "USDC:GBAD",
            "TOOLONGCODE123:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN",
        ] {
            assert!(validate_asset(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn builds_the_muxed_address_wallets_pay_to() {
        // Matches the M address stellar-sdk produced for this account and id
        // when the testnet muxed payment fixture was sent.
        assert_eq!(
            muxed_address(
                "GBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMRP7K",
                100_043
            )
            .unwrap(),
            "MBTF2Z62VJD4B54NGIS6JTGNPVH2O5HQNQF4S75NHVZIBP4JONQMQAAAAAAAAAMGZM3LK"
        );
        assert!(muxed_address("CABC", 1).is_err());
    }
}
