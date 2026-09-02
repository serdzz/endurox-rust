use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use endurox_rs::{
    ubf_fields, ubf_read_adhoc, ubf_read_nested, ubf_write_nested, AtmiCtx, BFldLocInfo,
    TypedBuffer, TypedUbf, UbfCarray, UbfDeserialize, UbfFieldDeserialize, UbfFieldSerialize,
    UbfFieldType, UbfGetValue, UbfResult, UbfSerialize, UbfValue,
};

#[test]
fn ubf_change_and_get_scalar_fields() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_SHORT_FLD, 0, UbfValue::Short(123), false)
        .expect("short Bchg failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(456789), false)
        .expect("long Bchg failed");
    ubf.bchg(ubf_fields::T_CHAR_FLD, 0, UbfValue::Char(b'Z' as i8), false)
        .expect("char Bchg failed");
    ubf.bchg(ubf_fields::T_FLOAT_FLD, 0, UbfValue::Float(12.5), false)
        .expect("float Bchg failed");
    ubf.bchg(ubf_fields::T_DOUBLE_FLD, 0, UbfValue::Double(123.75), false)
        .expect("double Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("hello-ubf".to_string()),
        false,
    )
    .expect("string Bchg failed");

    assert_eq!(ubf.bget_short(ubf_fields::T_SHORT_FLD, 0).unwrap(), 123);
    assert_eq!(ubf.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 456789);
    assert_eq!(
        ubf.bget_char(ubf_fields::T_CHAR_FLD, 0).unwrap(),
        b'Z' as i8
    );
    assert!((ubf.bget_float(ubf_fields::T_FLOAT_FLD, 0).unwrap() - 12.5).abs() < f32::EPSILON);
    assert!((ubf.bget_double(ubf_fields::T_DOUBLE_FLD, 0).unwrap() - 123.75).abs() < f64::EPSILON);
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "hello-ubf"
    );
}

#[test]
fn ubf_change_and_get_carray_field() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");
    let bytes = vec![0, 1, 2, 3, 254, 255];

    ubf.bchg(
        ubf_fields::T_CARRAY_FLD,
        0,
        UbfValue::Carray(bytes.clone()),
        false,
    )
    .expect("carray Bchg failed");

    assert_eq!(ubf.bget_bytes(ubf_fields::T_CARRAY_FLD, 0).unwrap(), bytes);
}

#[test]
fn ubf_change_and_get_embedded_ubf_field() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut outer = ctx.tpalloc_ubf(4096).expect("outer tpalloc_ubf failed");
    let mut inner = ctx.tpalloc_ubf(1024).expect("inner tpalloc_ubf failed");

    inner
        .bchg(
            ubf_fields::T_STRING_3_FLD,
            0,
            UbfValue::String("inside".to_string()),
            false,
        )
        .expect("inner string Bchg failed");
    outer
        .bchg(ubf_fields::T_UBF_FLD, 0, UbfValue::Ubf(inner), false)
        .expect("embedded UBF Bchg failed");

    let borrowed = outer.bget_ubf(ubf_fields::T_UBF_FLD, 0).unwrap();
    assert_eq!(
        borrowed.bget_string(ubf_fields::T_STRING_3_FLD, 0).unwrap(),
        "inside"
    );
}

#[test]
fn ubf_multiple_occurrences_are_indexed() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("first".to_string()),
        false,
    )
    .expect("first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        1,
        UbfValue::String("second".to_string()),
        false,
    )
    .expect("second occurrence Bchg failed");

    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "first"
    );
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 1).unwrap(),
        "second"
    );
}

#[test]
fn ubf_add_and_into_value_helpers_append_occurrences() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.badd(ubf_fields::T_STRING_FLD, "first", false)
        .expect("first Badd failed");
    ubf.badd(ubf_fields::T_STRING_FLD, String::from("second"), false)
        .expect("second Badd failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, 42_i64, false)
        .expect("typed long Bchg failed");

    assert_eq!(ctx.boccur(&ubf, ubf_fields::T_STRING_FLD).unwrap(), 2);
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "first"
    );
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 1).unwrap(),
        "second"
    );
    assert_eq!(ubf.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 42);
}

#[derive(Debug, PartialEq)]
struct SerdeChild {
    code: i64,
    note: String,
}

impl UbfSerialize for SerdeChild {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()> {
        self.code
            .ubf_write_field(ubf, ubf_fields::T_LONG_3_FLD, 0, realloc)?;
        self.note
            .ubf_write_field(ubf, ubf_fields::T_STRING_3_FLD, 0, realloc)
    }
}

impl UbfDeserialize for SerdeChild {
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self> {
        Ok(Self {
            code: i64::ubf_read_field(ubf, ubf_fields::T_LONG_3_FLD, 0)?,
            note: String::ubf_read_field(ubf, ubf_fields::T_STRING_3_FLD, 0)?,
        })
    }
}

#[derive(Debug, PartialEq)]
struct SerdeParent {
    name: String,
    values: Vec<i64>,
    maybe_note: Option<String>,
    blobs: Vec<UbfCarray>,
    child: SerdeChild,
    adhoc_note: String,
}

impl UbfSerialize for SerdeParent {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()> {
        self.name
            .ubf_write_field(ubf, ubf_fields::T_STRING_FLD, 0, realloc)?;
        self.values
            .ubf_write_field(ubf, ubf_fields::T_LONG_FLD, 0, realloc)?;
        self.maybe_note
            .ubf_write_field(ubf, ubf_fields::T_STRING_2_FLD, 0, realloc)?;
        self.blobs
            .ubf_write_field(ubf, ubf_fields::T_CARRAY_FLD, 0, realloc)?;
        ubf_write_nested(ubf, ubf_fields::T_UBF_FLD, 0, &self.child, 1024, realloc)?;

        let mut adhoc = ubf.ctx().tpalloc_ubf(1024).map_err(|e| {
            endurox_rs::UbfError::new(
                endurox_rs::UbfError::BMALLOC,
                format!("failed to allocate ad-hoc UBF: {}", e.message),
            )
        })?;
        adhoc.bchg(
            ubf_fields::T_STRING_4_FLD,
            0,
            self.adhoc_note.as_str(),
            realloc,
        )?;
        ubf.bchg(ubf_fields::T_UBF_2_FLD, 0, adhoc, realloc)
    }
}

impl UbfDeserialize for SerdeParent {
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self> {
        Ok(Self {
            name: String::ubf_read_field(ubf, ubf_fields::T_STRING_FLD, 0)?,
            values: Vec::<i64>::ubf_read_field(ubf, ubf_fields::T_LONG_FLD, 0)?,
            maybe_note: Option::<String>::ubf_read_field(ubf, ubf_fields::T_STRING_2_FLD, 0)?,
            blobs: Vec::<UbfCarray>::ubf_read_field(ubf, ubf_fields::T_CARRAY_FLD, 0)?,
            child: ubf_read_nested(ubf, ubf_fields::T_UBF_FLD, 0)?,
            adhoc_note: ubf_read_adhoc(ubf, ubf_fields::T_UBF_2_FLD, 0, |nested| {
                nested.bget_string(ubf_fields::T_STRING_4_FLD, 0)
            })?,
        })
    }
}

#[test]
fn ubf_serde_runtime_maps_repeated_optional_and_nested_fields() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    let source = SerdeParent {
        name: "parent".to_string(),
        values: vec![10, 20, 30],
        maybe_note: Some("optional".to_string()),
        blobs: vec![UbfCarray(vec![1, 2, 3]), UbfCarray(vec![4, 5])],
        child: SerdeChild {
            code: 777,
            note: "nested".to_string(),
        },
        adhoc_note: "free-form nested".to_string(),
    };

    ubf.ubf_write(&source, true).expect("UBF serialize failed");
    let decoded: SerdeParent = ubf.ubf_read().expect("UBF deserialize failed");

    assert_eq!(decoded, source);
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct TaggedSerdeChild {
    #[ubf(field = ubf_fields::T_LONG_3_FLD)]
    code: i64,
    #[ubf(field = ubf_fields::T_STRING_3_FLD)]
    note: String,
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct TaggedSerdeParent {
    #[ubf(field = ubf_fields::T_STRING_FLD)]
    name: String,
    #[ubf(field = ubf_fields::T_LONG_FLD)]
    values: Vec<i64>,
    #[ubf(field = ubf_fields::T_STRING_2_FLD)]
    maybe_note: Option<String>,
    #[ubf(field = ubf_fields::T_CARRAY_FLD)]
    blobs: Vec<UbfCarray>,
    #[ubf(field = ubf_fields::T_UBF_FLD, nested, size = 1024)]
    child: TaggedSerdeChild,
}

#[test]
fn ubf_serde_derive_uses_field_tags_and_nested_structs() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    let source = TaggedSerdeParent {
        name: "tagged".to_string(),
        values: vec![100, 200],
        maybe_note: None,
        blobs: vec![UbfCarray(vec![9, 8, 7])],
        child: TaggedSerdeChild {
            code: 321,
            note: "tagged child".to_string(),
        },
    };

    ubf.ubf_write(&source, true)
        .expect("derive serialize failed");

    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "tagged"
    );
    assert_eq!(ubf.bget_long(ubf_fields::T_LONG_FLD, 1).unwrap(), 200);
    assert!(!ubf.ctx().bpres(&ubf, ubf_fields::T_STRING_2_FLD, 0));

    let decoded: TaggedSerdeParent = ubf.ubf_read().expect("derive deserialize failed");
    assert_eq!(decoded, source);
}

#[test]
fn ubf_dynamic_get_and_fast_add_match_field_types() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");
    let mut loc = BFldLocInfo::default();

    ubf.badd_fast(ubf_fields::T_SHORT_FLD, 5_i16, &mut loc, true, false)
        .expect("short Baddfast failed");
    ubf.badd_fast(ubf_fields::T_SHORT_FLD, 7_i16, &mut loc, false, false)
        .expect("second short Baddfast failed");
    ubf.bchg(ubf_fields::T_STRING_FLD, 0, "dynamic", false)
        .expect("string Bchg failed");

    match ubf
        .bget(ubf_fields::T_SHORT_FLD, 1)
        .expect("dynamic short Bget failed")
    {
        UbfGetValue::Short(v) => assert_eq!(v, 7),
        _ => panic!("expected short dynamic value"),
    }

    match ubf
        .bget(ubf_fields::T_STRING_FLD, 0)
        .expect("dynamic string Bget failed")
    {
        UbfGetValue::String(v) => assert_eq!(v, "dynamic"),
        _ => panic!("expected string dynamic value"),
    }
}

#[test]
fn ubf_change_combined_and_binit_cover_go_parity_helpers() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg_combined(ubf_fields::T_STRING_FLD, 0, "combined", true, false)
        .expect("BChgCombined add failed");
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "combined"
    );

    ubf.bchg_combined(ubf_fields::T_STRING_FLD, 0, "changed", false, false)
        .expect("BChgCombined change failed");
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "changed"
    );

    let size = ubf.bsizeof().expect("Bsizeof failed");
    ctx.binit(&mut ubf, size).expect("Binit failed");
    assert!(!ctx.bpres(&ubf, ubf_fields::T_STRING_FLD, 0));
}

#[test]
fn ubf_buffer_iteration_reports_fields_and_occurrences() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_STRING_FLD, 0, "iter-one", false)
        .expect("string Bchg failed");
    ubf.bchg(ubf_fields::T_STRING_FLD, 1, "iter-two", false)
        .expect("second string Bchg failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, 777_i64, false)
        .expect("long Bchg failed");

    let mut seen = Vec::new();
    let mut iter = ubf.bnext();
    while let Some(field) = iter.next().expect("Bnext failed") {
        seen.push((field.field_id, field.occurrence, field.field_type));
        assert!(field.len > 0);
    }

    assert!(seen.contains(&(ubf_fields::T_STRING_FLD, 0, UbfFieldType::String)));
    assert!(seen.contains(&(ubf_fields::T_STRING_FLD, 1, UbfFieldType::String)));
    assert!(seen.contains(&(ubf_fields::T_LONG_FLD, 0, UbfFieldType::Long)));
}

#[test]
fn ubf_missing_field_returns_error() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");

    let err = ubf
        .bget_string(ubf_fields::T_STRING_FLD, 0)
        .expect_err("missing field should fail");
    assert_ne!(err.code, 0);
}

#[test]
fn ubf_buffer_can_be_reallocated() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    let original_size = ubf.bsizeof().expect("Bsizeof failed");

    ubf.tprealloc(original_size * 2).expect("tprealloc failed");
    let new_size = ubf.bsizeof().expect("Bsizeof after tprealloc failed");

    assert!(new_size >= original_size * 2);
}

#[test]
fn ubf_generic_buffer_can_be_cast_to_ubf() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let generic: TypedBuffer<'_> = ctx.tpalloc("UBF", "", 2048).expect("tpalloc failed");
    let mut ubf = TypedUbf::from_typed(generic);

    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        0,
        UbfValue::String("cast-ok".to_string()),
        false,
    )
    .expect("Bchg on cast buffer failed");

    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_2_FLD, 0).unwrap(),
        "cast-ok"
    );
}

#[test]
fn atmictx_ubf_presence_count_length_and_delete_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("first".to_string()),
        false,
    )
    .expect("first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_FLD,
        1,
        UbfValue::String("second".to_string()),
        false,
    )
    .expect("second occurrence Bchg failed");
    ubf.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(99), false)
        .expect("long Bchg failed");

    assert!(ctx.bpres(&ubf, ubf_fields::T_STRING_FLD, 0));
    assert_eq!(ctx.boccur(&ubf, ubf_fields::T_STRING_FLD).unwrap(), 2);
    assert!(ctx.blen(&ubf, ubf_fields::T_STRING_FLD, 0).unwrap() >= "first".len());
    assert!(ctx.bnum(&ubf).unwrap() >= 3);
    assert!(ctx.bisubf(&ubf));
    assert!(ctx.bsizeof(&ubf).unwrap() > 0);
    assert!(ctx.bused(&ubf).unwrap() > 0);
    assert!(ctx.bunused(&ubf).unwrap() > 0);

    ctx.bdel(&mut ubf, ubf_fields::T_STRING_FLD, 0)
        .expect("Bdel failed");
    assert_eq!(
        ubf.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "second"
    );

    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        0,
        UbfValue::String("delete-all-1".to_string()),
        false,
    )
    .expect("delete-all first occurrence Bchg failed");
    ubf.bchg(
        ubf_fields::T_STRING_2_FLD,
        1,
        UbfValue::String("delete-all-2".to_string()),
        false,
    )
    .expect("delete-all second occurrence Bchg failed");
    ctx.bdelall(&mut ubf, ubf_fields::T_STRING_2_FLD)
        .expect("Bdelall failed");
    assert!(!ctx.bpres(&ubf, ubf_fields::T_STRING_2_FLD, 0));

    let mut delete_list = [ubf_fields::T_LONG_FLD, 0];
    ctx.bdelete(&mut ubf, &mut delete_list)
        .expect("Bdelete failed");
    assert!(!ctx.bpres(&ubf, ubf_fields::T_LONG_FLD, 0));
}

#[test]
fn atmictx_ubf_copy_compare_project_and_subset_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut src = ctx.tpalloc_ubf(4096).expect("source tpalloc_ubf failed");
    let mut dst = ctx.tpalloc_ubf(4096).expect("dest tpalloc_ubf failed");
    let mut projected = ctx.tpalloc_ubf(4096).expect("projected tpalloc_ubf failed");

    src.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("keep".to_string()),
        false,
    )
    .expect("source string Bchg failed");
    src.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(1234), false)
        .expect("source long Bchg failed");

    ctx.bcpy(&mut dst, &src).expect("Bcpy failed");
    assert!(ctx.bcmp(&src, &dst));
    assert!(ctx.bsubset(&src, &dst));

    let mut project_list = [ubf_fields::T_STRING_FLD, 0];
    ctx.bprojcpy(&mut projected, &src, &mut project_list)
        .expect("Bprojcpy failed");
    assert_eq!(
        projected.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "keep"
    );
    assert!(!ctx.bpres(&projected, ubf_fields::T_LONG_FLD, 0));

    let mut in_place_project_list = [ubf_fields::T_LONG_FLD, 0];
    ctx.bproj(&mut dst, &mut in_place_project_list)
        .expect("Bproj failed");
    assert!(!ctx.bpres(&dst, ubf_fields::T_STRING_FLD, 0));
    assert_eq!(dst.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 1234);
}

#[test]
fn atmictx_ubf_update_concat_join_and_index_apis() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut dst = ctx.tpalloc_ubf(4096).expect("dest tpalloc_ubf failed");
    let mut src = ctx.tpalloc_ubf(4096).expect("source tpalloc_ubf failed");

    dst.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("old".to_string()),
        false,
    )
    .expect("dest string Bchg failed");
    src.bchg(
        ubf_fields::T_STRING_FLD,
        0,
        UbfValue::String("new".to_string()),
        false,
    )
    .expect("source string Bchg failed");
    src.bchg(ubf_fields::T_LONG_FLD, 0, UbfValue::Long(77), false)
        .expect("source long Bchg failed");

    ctx.bupdate(&mut dst, &src).expect("Bupdate failed");
    assert_eq!(dst.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(), "new");
    assert_eq!(dst.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(), 77);

    ctx.bconcat(&mut dst, &src).expect("Bconcat failed");
    assert!(ctx.boccur(&dst, ubf_fields::T_STRING_FLD).unwrap() >= 2);

    let mut joined = ctx.tpalloc_ubf(4096).expect("joined tpalloc_ubf failed");
    joined
        .bchg(
            ubf_fields::T_STRING_FLD,
            0,
            UbfValue::String("join-target".to_string()),
            false,
        )
        .expect("joined seed Bchg failed");
    ctx.bjoin(&mut joined, &src).expect("Bjoin failed");
    assert_eq!(
        joined.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "new"
    );

    let mut outer_joined = ctx
        .tpalloc_ubf(4096)
        .expect("outer joined tpalloc_ubf failed");
    outer_joined
        .bchg(
            ubf_fields::T_STRING_2_FLD,
            0,
            UbfValue::String("kept".to_string()),
            false,
        )
        .expect("outer joined seed Bchg failed");
    ctx.bojoin(&mut outer_joined, &src).expect("Bojoin failed");
    assert_eq!(
        outer_joined
            .bget_string(ubf_fields::T_STRING_2_FLD, 0)
            .unwrap(),
        "kept"
    );

    ctx.bindex(&mut dst, 0).expect("Bindex failed");
    let _ = ctx.bidxused(&dst).expect("Bidxused failed");
    let _ = ctx.bunindex(&mut dst).expect("Bunindex failed");
}

#[test]
fn atmictx_ubf_field_id_helper_uses_defined_types() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");

    assert_eq!(
        ctx.bmkfldid_typed(UbfFieldType::String, 1061),
        ubf_fields::T_STRING_FLD
    );
    assert_eq!(
        ctx.bmkfldid_typed(UbfFieldType::Long, 1031),
        ubf_fields::T_LONG_FLD
    );
    assert_eq!(
        ctx.bfldid("T_STRING_FLD").unwrap(),
        ubf_fields::T_STRING_FLD
    );
    assert_eq!(
        ctx.bfname(ubf_fields::T_STRING_FLD).unwrap(),
        "T_STRING_FLD"
    );
    assert_eq!(ctx.bfldno(ubf_fields::T_STRING_FLD), 1061);
    assert_eq!(
        ctx.bfldtype(ubf_fields::T_STRING_FLD).unwrap(),
        UbfFieldType::String
    );
    assert_eq!(ctx.btype(ubf_fields::T_STRING_FLD).unwrap(), "string");
    assert_eq!(ctx.bmkfldid(5, 1061).unwrap(), ubf_fields::T_STRING_FLD);
}

#[test]
fn ubf_print_extread_and_binary_roundtrip() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut src = ctx.tpalloc_ubf(4096).expect("source tpalloc_ubf failed");
    let mut from_text = ctx.tpalloc_ubf(4096).expect("text tpalloc_ubf failed");
    let mut from_dump = ctx.tpalloc_ubf(4096).expect("dump tpalloc_ubf failed");

    src.bchg(ubf_fields::T_STRING_FLD, 0, "printed", false)
        .expect("string Bchg failed");
    src.bchg(ubf_fields::T_LONG_FLD, 0, 12345_i64, false)
        .expect("long Bchg failed");

    let printed = src.bsprint().expect("BSprint failed");
    assert!(printed.contains("printed"));

    from_text.bextread(&printed).expect("BExtRead failed");
    assert_eq!(
        from_text.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "printed"
    );
    assert_eq!(
        from_text.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(),
        12345
    );

    let dump = src.bwrite().expect("BWrite failed");
    assert!(!dump.is_empty());
    from_dump.bread(&dump).expect("BRead failed");
    assert_eq!(
        from_dump.bget_string(ubf_fields::T_STRING_FLD, 0).unwrap(),
        "printed"
    );
    assert_eq!(
        from_dump.bget_long(ubf_fields::T_LONG_FLD, 0).unwrap(),
        12345
    );
}

#[test]
fn ubf_boolean_expression_compile_eval_and_print() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_LONG_FLD, 0, 77_i64, false)
        .expect("long Bchg failed");
    ctx.bboolsetcbf("rust_long_is_77", rust_long_is_77)
        .expect("Bboolsetcbf failed");

    let tree = ctx.bboolco("rust_long_is_77()").expect("Bboolco failed");
    assert!(ubf.bboolev(&tree));
    assert!(ubf.bqboolev("rust_long_is_77()").expect("BQBoolEv failed"));

    let printed = ctx.bboolpr(&tree).expect("Bboolpr failed");
    assert!(!printed.is_empty());
    assert_eq!(ubf.bfloatev(&tree), 1.0);

    ctx.btreefree(tree);
}

#[test]
fn ubf_boolean_expression_callbacks_are_invoked() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_LONG_FLD, 0, 77_i64, false)
        .expect("long Bchg failed");

    ctx.bboolsetcbf("rust_long_is_77", rust_long_is_77)
        .expect("Bboolsetcbf failed");
    ctx.bboolsetcbf2("rust_long_arg", rust_long_arg)
        .expect("Bboolsetcbf2 failed");

    assert!(ubf
        .bqboolev("rust_long_is_77()")
        .expect("callback expression failed"));
    assert!(ubf
        .bqboolev("rust_long_arg('77')")
        .expect("callback expression with arg failed"));
}

#[test]
fn ubf_print_wrappers_are_callable() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(4096).expect("tpalloc_ubf failed");

    ubf.bchg(ubf_fields::T_STRING_FLD, 0, "printable", false)
        .expect("string Bchg failed");

    ubf.bprint().expect("BPrint failed");
    ubf.tplogprintubf(1, "rust ubf print test")
        .expect("TpLogPrintUBF failed");
}

fn rust_long_is_77(ubf: &TypedUbf<'_>, _funcname: &str) -> i64 {
    (ubf.bget_long(ubf_fields::T_LONG_FLD, 0).ok() == Some(77)) as i64
}

fn rust_long_arg(ubf: &TypedUbf<'_>, _funcname: &str, arg: &str) -> i64 {
    let expected = arg.parse::<i64>().unwrap_or_default();
    (ubf.bget_long(ubf_fields::T_LONG_FLD, 0).ok() == Some(expected)) as i64
}

/// A `BFLD_PTR` field must store the target's *address*, not the first bytes of
/// its contents, and the target must survive the write.
///
/// `CBchg` takes a pointer to the value; for `BFLD_PTR` the value is itself a
/// pointer. Passing the target address directly used to store the target's
/// payload bytes as if they were an address.
#[test]
fn ubf_ptr_field_round_trips_the_target_buffer() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    let target = ctx
        .tpalloc_carray(b"PAYLOAD")
        .expect("tpalloc_carray failed");

    master
        .bchg(ubf_fields::T_PTR_FLD, 0, UbfValue::Ptr(target), true)
        .expect("storing a BFLD_PTR field failed");

    // tptypes only succeeds for a buffer Enduro/X still has registered, so this
    // proves both that the address round-tripped and that the target was not
    // freed when `bchg` consumed the wrapper.
    let borrowed = master
        .bget_ptr(ubf_fields::T_PTR_FLD, 0)
        .expect("bget_ptr failed");
    let info = borrowed
        .tptypes()
        .expect("target is not a live ATMI buffer");
    assert_eq!(info.type_name, "CARRAY");
    assert_eq!(info.size, 7);

    drop(borrowed);
    // Dropping `master` frees the target through Enduro/X's cascade
    // (ndrx_tpfree_scan_ptrs). Freeing it here as well would be a double free.
    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

/// Extracting a `BFLD_PTR` removes the reference, so the master no longer
/// cascades into it and the caller becomes its sole owner.
#[test]
fn ubf_ptr_field_extraction_transfers_ownership() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(1024).expect("master");
    let target = ctx.tpalloc_ubf(512).expect("target").into_inner();
    master
        .bchg(ubf_fields::T_PTR_2_FLD, 0, UbfValue::Ptr(target), true)
        .expect("storing a BFLD_PTR field failed");

    let extracted = master
        .bextract_ptr(ubf_fields::T_PTR_2_FLD, 0)
        .expect("bextract_ptr failed");
    assert!(
        master.bget_ptr(ubf_fields::T_PTR_2_FLD, 0).is_err(),
        "extraction must remove the occurrence from the master"
    );

    // The master must not free the extracted buffer.
    drop(master);

    // Still live, and writable -- extraction hands over a standalone buffer.
    let mut owned = TypedUbf::from_typed(extracted);
    owned
        .bchg(
            ubf_fields::T_STRING_FLD,
            0,
            UbfValue::String("ALIVE".to_string()),
            true,
        )
        .expect("extracted buffer should be writable after the master is gone");
    assert_eq!(
        owned
            .bget_string(ubf_fields::T_STRING_FLD, 0)
            .expect("read back"),
        "ALIVE"
    );

    drop(owned);
    ctx.tpterm().expect("tpterm failed");
}

/// `BFLD_PTR` is a normal multi-occurrence field: each occurrence holds its own
/// target, and extracting one must not disturb the others.
#[test]
fn ubf_ptr_field_supports_multiple_occurrences() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(4096).expect("master");

    // Distinct sizes so each occurrence is identifiable via tptypes().
    for size in [3usize, 5, 9] {
        let target = ctx
            .tpalloc_carray(&vec![b'x'; size])
            .expect("tpalloc_carray failed");
        master
            .badd(ubf_fields::T_PTR_FLD, UbfValue::Ptr(target), true)
            .expect("badd BFLD_PTR occurrence failed");
    }

    for (occ, expect) in [(0, 3usize), (1, 5), (2, 9)] {
        let got = master
            .bget_ptr(ubf_fields::T_PTR_FLD, occ)
            .expect("bget_ptr on occurrence failed")
            .tptypes()
            .expect("occurrence target is not a live ATMI buffer");
        assert_eq!(got.type_name, "CARRAY");
        assert_eq!(
            got.size, expect,
            "occurrence {occ} resolved to the wrong target"
        );
    }

    // Extract the middle one; UBF closes the gap, so occ 1 becomes the old occ 2.
    let extracted = master
        .bextract_ptr(ubf_fields::T_PTR_FLD, 1)
        .expect("bextract_ptr failed");
    assert_eq!(extracted.tptypes().expect("extracted live").size, 5);

    assert_eq!(
        master
            .bget_ptr(ubf_fields::T_PTR_FLD, 0)
            .expect("occ 0 survives")
            .tptypes()
            .expect("live")
            .size,
        3
    );
    assert_eq!(
        master
            .bget_ptr(ubf_fields::T_PTR_FLD, 1)
            .expect("occ 2 shifted down to occ 1")
            .tptypes()
            .expect("live")
            .size,
        9
    );
    assert!(
        master.bget_ptr(ubf_fields::T_PTR_FLD, 2).is_err(),
        "only two occurrences should remain"
    );

    // master frees occurrences 0 and 1; `extracted` frees itself.
    drop(master);
    drop(extracted);
    ctx.tpterm().expect("tpterm failed");
}

/// `bget_ptr_ubf` gives a read-only view of a UBF target and refuses to
/// reinterpret a target of any other buffer type as UBF.
#[test]
fn ubf_ptr_field_read_only_ubf_view() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(4096).expect("master");

    // occ 0: a UBF target carrying a known field.
    let mut inner = ctx.tpalloc_ubf(1024).expect("inner ubf");
    inner
        .bchg(
            ubf_fields::T_STRING_FLD,
            0,
            UbfValue::String("NESTED".to_string()),
            true,
        )
        .expect("write into the target");
    master
        .badd(
            ubf_fields::T_PTR_FLD,
            UbfValue::Ptr(inner.into_inner()),
            true,
        )
        .expect("badd ubf target");

    // occ 1: a CARRAY target, which must not pass as UBF.
    let carray = ctx.tpalloc_carray(b"NOTUBF").expect("carray target");
    master
        .badd(ubf_fields::T_PTR_FLD, UbfValue::Ptr(carray), true)
        .expect("badd carray target");

    let view = master
        .bget_ptr_ubf(ubf_fields::T_PTR_FLD, 0)
        .expect("bget_ptr_ubf on a UBF target failed");
    assert_eq!(
        view.bget_string(ubf_fields::T_STRING_FLD, 0)
            .expect("read through the read-only view"),
        "NESTED"
    );
    // `view` borrows `master`; letting it fall out of scope here releases the
    // borrow. An explicit drop() is redundant -- BorrowedUbf owns nothing.

    let err = master
        .bget_ptr_ubf(ubf_fields::T_PTR_FLD, 1)
        .expect_err("a CARRAY target must not be handed out as a UBF view");
    assert_eq!(err.code, endurox_rs::UbfError::BTYPERR);

    // bget_ptr still reaches the same CARRAY target.
    assert_eq!(
        master
            .bget_ptr(ubf_fields::T_PTR_FLD, 1)
            .expect("bget_ptr on the carray target")
            .tptypes()
            .expect("live")
            .type_name,
        "CARRAY"
    );

    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

/// `badd_fast` must transfer `BFLD_PTR` ownership exactly like the normal write
/// path. It returns early on success, so it needs its own `mem::forget`;
/// without it the target is freed the moment the call returns and the field is
/// left pointing at freed memory.
#[test]
fn ubf_ptr_field_fast_add_transfers_ownership() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(4096).expect("master");
    let mut loc = BFldLocInfo::default();

    let first = ctx.tpalloc_carray(b"FAST-1").expect("first target");
    master
        .badd_fast(
            ubf_fields::T_PTR_FLD,
            UbfValue::Ptr(first),
            &mut loc,
            true,
            true,
        )
        .expect("badd_fast BFLD_PTR failed");

    let second = ctx.tpalloc_carray(b"FAST-22").expect("second target");
    master
        .badd_fast(
            ubf_fields::T_PTR_FLD,
            UbfValue::Ptr(second),
            &mut loc,
            false,
            true,
        )
        .expect("second badd_fast BFLD_PTR failed");

    // tptypes only succeeds while Enduro/X still has the buffer registered, so
    // this fails loudly if either target was freed by the fast path.
    for (occ, size) in [(0, 6usize), (1, 7)] {
        let info = master
            .bget_ptr(ubf_fields::T_PTR_FLD, occ)
            .expect("bget_ptr after badd_fast")
            .tptypes()
            .expect("fast-added target was freed while still referenced");
        assert_eq!(info.type_name, "CARRAY");
        assert_eq!(info.size, size);
    }

    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

/// Overwriting an occupied `BFLD_PTR` occurrence must reclaim the target it
/// displaces. Nothing else references it afterwards, so Enduro/X's free cascade
/// would never reach it and it would leak for the life of the process.
#[test]
fn ubf_ptr_field_replacement_reclaims_the_old_target() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    ctx.tpinit().expect("tpinit failed");

    let mut master = ctx.tpalloc_ubf(4096).expect("master");

    let first = ctx.tpalloc_carray(b"OLD").expect("first target");
    master
        .bchg(ubf_fields::T_PTR_FLD, 0, UbfValue::Ptr(first), true)
        .expect("initial bchg failed");

    // Replace it. The displaced target must be freed, not orphaned.
    let second = ctx.tpalloc_carray(b"REPLACEMENT").expect("second target");
    master
        .bchg(ubf_fields::T_PTR_FLD, 0, UbfValue::Ptr(second), true)
        .expect("replacing bchg failed");

    // The field now resolves to the replacement, still live.
    let info = master
        .bget_ptr(ubf_fields::T_PTR_FLD, 0)
        .expect("bget_ptr after replacement")
        .tptypes()
        .expect("replacement target should be live");
    assert_eq!(info.type_name, "CARRAY");
    assert_eq!(info.size, 11);

    // Dropping the master frees only the replacement; the displaced buffer was
    // already reclaimed, so this must not double-free.
    drop(master);
    ctx.tpterm().expect("tpterm failed");
}

#[test]
fn atmictx_ubf_error_paths_set_ubf_error() {
    let _guard = endurox_test_env();
    let ctx = AtmiCtx::new().expect("failed to create AtmiCtx");
    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");

    let err = ctx
        .bdel(&mut ubf, ubf_fields::T_STRING_FLD, 0)
        .expect_err("Bdel on missing field should fail");
    assert_ne!(err.code, 0);

    let missing = ubf
        .bget_long(ubf_fields::T_LONG_FLD, 0)
        .expect_err("missing long should fail");
    assert_ne!(missing.code, 0);
}

fn endurox_test_env() -> MutexGuard<'static, ()> {
    let guard = match endurox_test_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    provision_endurox_env();
    guard
}

fn endurox_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn provision_endurox_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let test_dir = manifest_dir.join("tests").join("00_unittest");
        let ubf_file = manifest_dir.join("tests").join("ubftab").join("test.fd");

        let output = Command::new("bash")
            .arg("-lc")
            .arg(
                r#"
set -euo pipefail
cd "$NDRX_RS_UNIT_TEST_DIR"
if [ -f "$HOME/ndrx_home" ]; then
    export CDPATH="${CDPATH:-}"
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
    export DYLD_FALLBACK_LIBRARY_PATH="${DYLD_FALLBACK_LIBRARY_PATH:-}"
    . "$HOME/ndrx_home"
fi
rm -f conf/app.ini conf/settest1
mkdir -p log
find log -type f -exec rm -f {} +
xadmin provision -d -vaddubf="$NDRX_RS_UNIT_UBF_FILE" >/dev/null
python3 -c "
import os, re
path = os.environ['NDRX_RS_UNIT_UBF_FILE']
d = os.path.dirname(path)
b = os.path.basename(path)
with open('conf/app.ini') as f:
    txt = f.read()
txt = re.sub(r'FIELDTBLS=Exfields,[^\n]+', 'FIELDTBLS=Exfields,' + b, txt)
txt = re.sub(r'(FLDTBLDIR=[^\n]+)', r'\1:' + d, txt)
with open('conf/app.ini', 'w') as f:
    f.write(txt)
"
. conf/settest1
export NDRX_CONFIG="$NDRX_RS_UNIT_TEST_DIR/conf/ndrxconfig.xml"
export FLDTBLDIR="$(dirname "$NDRX_RS_UNIT_UBF_FILE")"
export FIELDTBLS="$(basename "$NDRX_RS_UNIT_UBF_FILE")"
export NDRX_DEBUG_STR="file=$NDRX_RS_UNIT_TEST_DIR/log/ubf-tests.log ndrx=5"
env
"#,
            )
            .env("NDRX_RS_UNIT_TEST_DIR", &test_dir)
            .env("NDRX_RS_UNIT_UBF_FILE", &ubf_file)
            .output()
            .expect("failed to run xadmin provision for UBF tests");

        if !output.status.success() {
            panic!(
                "xadmin provision failed with status={}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((key, value)) = line.split_once('=') {
                env::set_var(key, value);
            }
        }
    });
}
