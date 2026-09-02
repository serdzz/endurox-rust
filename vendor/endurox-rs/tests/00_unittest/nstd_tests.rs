use endurox_rs::{AtmiCtx, NdrxStdCfgStr};

#[test]
fn ndrx_stdcfgstr_parse_mixed_keys_and_kv() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let parsed = ctx
        .ndrx_stdcfgstr_parse("HELLO,WORLD,,,,  THIS=VALUE")
        .expect("ndrx_stdcfgstr_parse failed");

    assert_eq!(
        parsed,
        vec![
            NdrxStdCfgStr {
                key: "HELLO".to_string(),
                value: None,
            },
            NdrxStdCfgStr {
                key: "WORLD".to_string(),
                value: None,
            },
            NdrxStdCfgStr {
                key: "THIS".to_string(),
                value: Some("VALUE".to_string()),
            },
        ]
    );
}

#[test]
fn ndrx_stdcfgstr_parse_whitespace_separators() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let parsed = ctx
        .ndrx_stdcfgstr_parse(",,,\nIS\tANOTHER=SETTING")
        .expect("ndrx_stdcfgstr_parse failed");

    assert_eq!(
        parsed,
        vec![
            NdrxStdCfgStr {
                key: "IS".to_string(),
                value: None,
            },
            NdrxStdCfgStr {
                key: "ANOTHER".to_string(),
                value: Some("SETTING".to_string()),
            },
        ]
    );
}

#[test]
fn ndrx_stdcfgstr_parse_quoted_values() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let parsed = ctx
        .ndrx_stdcfgstr_parse("X='=HELLO WORLD INSIDE\"' Y=\"HELO\\\" EHLO\" ndrx=5")
        .expect("ndrx_stdcfgstr_parse failed");

    assert_eq!(
        parsed,
        vec![
            NdrxStdCfgStr {
                key: "X".to_string(),
                value: Some("=HELLO WORLD INSIDE\"".to_string()),
            },
            NdrxStdCfgStr {
                key: "Y".to_string(),
                value: Some("HELO\" EHLO".to_string()),
            },
            NdrxStdCfgStr {
                key: "ndrx".to_string(),
                value: Some("5".to_string()),
            },
        ]
    );
}

#[test]
fn ndrx_stdcfgstr_parse_empty_input() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let parsed = ctx
        .ndrx_stdcfgstr_parse("")
        .expect("ndrx_stdcfgstr_parse failed");

    assert!(parsed.is_empty());
}

#[test]
fn ndrx_stdcfgstr_parse_rejects_nul_byte() {
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    let err = ctx
        .ndrx_stdcfgstr_parse("KEY=val\0ue")
        .expect_err("expected NUL-byte error");

    assert!(
        err.message.contains("NUL"),
        "unexpected message: {}",
        err.message
    );
}
