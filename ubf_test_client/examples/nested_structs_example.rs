/// Nested UBF structures with endurox-rs
///
/// Enduro/X UBF buffers can embed other UBF buffers as fields (field type
/// `ubf`). endurox-rs maps that onto plain nested Rust structs: mark the
/// field with `#[ubf(field = ..., nested)]` and derive
/// `UbfSerialize`/`UbfDeserialize` on both the outer and the inner type.
///
/// This example round-trips a two-level structure through a UBF buffer:
///
/// ```text
/// Customer
/// ├── name, id
/// ├── address:  Address  (embedded UBF in T_ADDRESS_FLD)
/// │   └── street, city, zip
/// └── contact:  Contact  (embedded UBF in T_CONTACT_FLD)
///     └── name, message
/// ```
///
/// Run with: cargo run --example nested_structs_example
///
/// No server is needed — everything happens client-side in the buffer —
/// but an Enduro/X environment must be configured (NDRX_CCONFIG etc.) so
/// that a context can be created and FLDTBLDIR/FIELDTBLS resolve the
/// test field table.
use endurox_rs::{ubf_read_adhoc, AtmiCtx, UbfResult};
use endurox_rs::{UbfDeserialize, UbfSerialize};

// Auto-generated UBF field constants (from ubftab/*.fd.h)
#[allow(dead_code)]
mod ubf_fields {
    include!(concat!(env!("OUT_DIR"), "/ubf_fields.rs"));
}
use ubf_fields::*;

/// Inner structure: stored as an embedded UBF under T_ADDRESS_FLD.
#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Address {
    #[ubf(field = T_STREET_FLD)]
    street: String,

    #[ubf(field = T_CITY_FLD)]
    city: String,

    #[ubf(field = T_ZIP_FLD)]
    zip: String,
}

/// Another inner structure: embedded under T_CONTACT_FLD.
#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Contact {
    #[ubf(field = T_NAME_FLD)]
    name: String,

    #[ubf(field = T_MESSAGE_FLD)]
    message: Option<String>, // optional: absent field maps to None
}

/// Outer structure. The `nested` attribute makes the derive serialize the
/// field into its own UBF buffer and embed it, instead of flattening its
/// fields into the parent.
#[derive(Debug, Clone, PartialEq, UbfSerialize, UbfDeserialize)]
struct Customer {
    #[ubf(field = T_NAME_FLD)]
    name: String,

    #[ubf(field = T_ID_FLD)]
    id: i64,

    #[ubf(field = T_ADDRESS_FLD, nested)]
    address: Address,

    #[ubf(field = T_CONTACT_FLD, nested)]
    contact: Contact,
}

fn main() -> UbfResult<()> {
    let ctx = AtmiCtx::new().expect("failed to create ATMI context");

    println!("=== Nested UBF structures example ===\n");

    // Note: the outer struct reuses T_NAME_FLD, and so does Contact. That
    // is fine — the nested copy lives inside its own embedded buffer, so
    // the two never collide.
    let customer = Customer {
        name: "Sergej".into(),
        id: 1001,
        address: Address {
            street: "Brivibas iela 1".into(),
            city: "Riga".into(),
            zip: "LV-1010".into(),
        },
        contact: Contact {
            name: "S. Lepin".into(),
            message: Some("Preferred contact: evenings".into()),
        },
    };
    println!("original: {:#?}\n", customer);

    // --- Serialize: Customer -> UBF (address/contact become embedded UBFs)
    let mut ubf = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    customer.ubf_serialize(&mut ubf, true)?;

    // What the wire sees: Bprint of the outer buffer shows the embedded
    // buffers as composite fields.
    println!("--- UBF buffer contents (Bprint) ---");
    ubf.bprint()?;
    println!();

    // --- Deserialize: UBF -> Customer (embedded UBFs -> nested structs)
    let restored = Customer::ubf_deserialize(&ubf)?;
    println!("restored: {:#?}\n", restored);

    assert_eq!(customer, restored, "round-trip must be lossless");
    println!("round-trip OK: original == restored\n");

    // --- Ad-hoc access: read one field out of an embedded UBF without
    // declaring a Rust schema for it. Useful when a service only needs a
    // corner of a large nested structure.
    let city = ubf_read_adhoc(&ubf, T_ADDRESS_FLD, 0, |address_ubf| {
        address_ubf.bget_string(T_CITY_FLD, 0)
    })?;
    println!("ad-hoc read of address.city: {}", city);

    // Optional fields in nested structs: absent means None.
    let anonymous = Customer {
        name: "Anon".into(),
        id: 0,
        address: customer.address.clone(),
        contact: Contact {
            name: "n/a".into(),
            message: None, // not set -> field absent in the embedded UBF
        },
    };
    let mut ubf2 = ctx.tpalloc_ubf(1024).expect("tpalloc_ubf failed");
    anonymous.ubf_serialize(&mut ubf2, true)?;
    let restored2 = Customer::ubf_deserialize(&ubf2)?;
    assert_eq!(restored2.contact.message, None);
    println!("optional nested field round-trips as None: OK");

    println!("\n=== done ===");
    Ok(())
}
