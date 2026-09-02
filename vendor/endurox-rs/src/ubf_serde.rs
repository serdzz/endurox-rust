use crate::{TypedUbf, UbfError, UbfResult, UbfValue};

/// Serialize a Rust structure into a UBF buffer.
///
/// This is the runtime layer intended for derive macros or hand-written
/// mappings. A derive can call [`UbfFieldSerialize::ubf_write_field`] for each
/// annotated field.
pub trait UbfSerialize {
    fn ubf_serialize<'ctx>(&self, ubf: &mut TypedUbf<'ctx>, realloc: bool) -> UbfResult<()>;
}

/// Deserialize a Rust structure from a UBF buffer.
pub trait UbfDeserialize: Sized {
    fn ubf_deserialize<'ctx>(ubf: &TypedUbf<'ctx>) -> UbfResult<Self>;
}

/// Serialize one Rust value to one or more occurrences of a UBF field.
pub trait UbfFieldSerialize {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()>;
}

/// Deserialize one Rust value from one or more occurrences of a UBF field.
pub trait UbfFieldDeserialize: Sized {
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self>;
}

/// Explicit CARRAY wrapper.
///
/// Plain `Vec<T>` is reserved for repeated field occurrences. Use this wrapper
/// when the UBF field itself is a `carray`/byte blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbfCarray(pub Vec<u8>);

/// Ad-hoc embedded UBF wrapper.
///
/// Use this for embedded UBF fields whose contents are intentionally not mapped
/// to a Rust sub-structure.
#[derive(Debug)]
pub struct UbfAdhoc<'ctx>(pub TypedUbf<'ctx>);

impl TypedUbf<'_> {
    pub fn ubf_write<T: UbfSerialize>(&mut self, value: &T, realloc: bool) -> UbfResult<()> {
        value.ubf_serialize(self, realloc)
    }

    pub fn ubf_read<T: UbfDeserialize>(&self) -> UbfResult<T> {
        T::ubf_deserialize(self)
    }
}

macro_rules! impl_scalar_field {
    ($ty:ty, $getter:ident) => {
        impl UbfFieldSerialize for $ty {
            fn ubf_write_field<'ctx>(
                &self,
                ubf: &mut TypedUbf<'ctx>,
                field_id: i32,
                occurrence: i32,
                realloc: bool,
            ) -> UbfResult<()> {
                ubf.bchg(field_id, occurrence, *self, realloc)
            }
        }

        impl UbfFieldDeserialize for $ty {
            fn ubf_read_field<'ctx>(
                ubf: &TypedUbf<'ctx>,
                field_id: i32,
                occurrence: i32,
            ) -> UbfResult<Self> {
                ubf.$getter(field_id, occurrence).map(|v| v as $ty)
            }
        }
    };
}

impl_scalar_field!(i8, bget_char);
impl_scalar_field!(i16, bget_short);
impl_scalar_field!(i32, bget_long);
impl_scalar_field!(i64, bget_long);
impl_scalar_field!(u8, bget_short);
impl_scalar_field!(u16, bget_short);
impl_scalar_field!(u32, bget_long);
impl_scalar_field!(u64, bget_long);
impl_scalar_field!(f32, bget_float);
impl_scalar_field!(f64, bget_double);

impl UbfFieldSerialize for String {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(field_id, occurrence, self.as_str(), realloc)
    }
}

impl UbfFieldSerialize for str {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(field_id, occurrence, self, realloc)
    }
}

impl UbfFieldDeserialize for String {
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_string(field_id, occurrence)
    }
}

impl UbfFieldSerialize for UbfCarray {
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        ubf.bchg(
            field_id,
            occurrence,
            UbfValue::Carray(self.0.clone()),
            realloc,
        )
    }
}

impl UbfFieldDeserialize for UbfCarray {
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        ubf.bget_bytes(field_id, occurrence).map(UbfCarray)
    }
}

impl<'value, T> UbfFieldSerialize for &'value T
where
    T: UbfFieldSerialize + ?Sized,
{
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        (*self).ubf_write_field(ubf, field_id, occurrence, realloc)
    }
}

impl<T> UbfFieldSerialize for Option<T>
where
    T: UbfFieldSerialize,
{
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        if let Some(value) = self {
            value.ubf_write_field(ubf, field_id, occurrence, realloc)?;
        }
        Ok(())
    }
}

impl<T> UbfFieldDeserialize for Option<T>
where
    T: UbfFieldDeserialize,
{
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        if ubf.ctx().bpres(ubf, field_id, occurrence) {
            T::ubf_read_field(ubf, field_id, occurrence).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl<T> UbfFieldSerialize for Vec<T>
where
    T: UbfFieldSerialize,
{
    fn ubf_write_field<'ctx>(
        &self,
        ubf: &mut TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
        realloc: bool,
    ) -> UbfResult<()> {
        for (idx, value) in self.iter().enumerate() {
            value.ubf_write_field(ubf, field_id, occurrence + idx as i32, realloc)?;
        }
        Ok(())
    }
}

impl<T> UbfFieldDeserialize for Vec<T>
where
    T: UbfFieldDeserialize,
{
    fn ubf_read_field<'ctx>(
        ubf: &TypedUbf<'ctx>,
        field_id: i32,
        occurrence: i32,
    ) -> UbfResult<Self> {
        let total = ubf.ctx().boccur(ubf, field_id)? as i32;
        let mut values = Vec::new();
        for occ in occurrence..total {
            values.push(T::ubf_read_field(ubf, field_id, occ)?);
        }
        Ok(values)
    }
}

impl<'ctx> UbfFieldSerialize for UbfAdhoc<'ctx> {
    fn ubf_write_field<'buf>(
        &self,
        _ubf: &mut TypedUbf<'buf>,
        _field_id: i32,
        _occurrence: i32,
        _realloc: bool,
    ) -> UbfResult<()> {
        Err(UbfError::new(
            UbfError::BEINVAL,
            "borrow UbfAdhoc mutably to serialize embedded UBF",
        ))
    }
}

impl<'ctx> UbfFieldSerialize for &mut UbfAdhoc<'ctx> {
    fn ubf_write_field<'buf>(
        &self,
        _ubf: &mut TypedUbf<'buf>,
        _field_id: i32,
        _occurrence: i32,
        _realloc: bool,
    ) -> UbfResult<()> {
        Err(UbfError::new(
            UbfError::BEINVAL,
            "mutable embedded UBF serialization requires ownership",
        ))
    }
}

/// Write an owned ad-hoc embedded UBF field.
pub fn ubf_write_adhoc<'ctx>(
    ubf: &mut TypedUbf<'ctx>,
    field_id: i32,
    occurrence: i32,
    value: UbfAdhoc<'ctx>,
    realloc: bool,
) -> UbfResult<()> {
    ubf.bchg(field_id, occurrence, value.0, realloc)
}

/// Write a nested Rust structure as an embedded UBF field.
pub fn ubf_write_nested<T: UbfSerialize>(
    ubf: &mut TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
    value: &T,
    initial_size: usize,
    realloc: bool,
) -> UbfResult<()> {
    let ctx = ubf.ctx();
    let mut nested = ctx.tpalloc_ubf(initial_size).map_err(|e| {
        UbfError::new(
            UbfError::BMALLOC,
            format!("failed to allocate nested UBF: {}", e.message),
        )
    })?;
    value.ubf_serialize(&mut nested, realloc)?;
    ubf.bchg(field_id, occurrence, nested, realloc)
}

/// Read a nested Rust structure from an embedded UBF field.
pub fn ubf_read_nested<T: UbfDeserialize>(
    ubf: &TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
) -> UbfResult<T> {
    let nested = ubf.bget_ubf(field_id, occurrence)?;
    let nested = unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr as *mut _) };
    T::ubf_deserialize(&nested)
}

/// Read an embedded UBF with a caller supplied mapper.
///
/// This is the ad-hoc escape hatch for sub-structures that need to inspect a
/// nested UBF without declaring a fixed Rust schema for it.
pub fn ubf_read_adhoc<R>(
    ubf: &TypedUbf<'_>,
    field_id: i32,
    occurrence: i32,
    f: impl FnOnce(&TypedUbf<'_>) -> UbfResult<R>,
) -> UbfResult<R> {
    let nested = ubf.bget_ubf(field_id, occurrence)?;
    let nested = unsafe { TypedUbf::borrowed_from_raw(nested.ctx, nested.ptr as *mut _) };
    f(&nested)
}
