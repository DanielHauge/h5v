use hdf5_metno::{
    datatype::Datatype,
    from_id,
    types::{Reference, TypeDescriptor},
    Attribute, Error,
};
use hdf5_metno_sys::h5t::{H5Tget_class, H5Tget_super, H5T_REFERENCE, H5T_VLEN};

fn detect_reference_descriptor(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    let object_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Object))?;
    if dtype == &object_ref {
        return Ok(TypeDescriptor::Reference(Reference::Object));
    }

    let region_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Region))?;
    if dtype == &region_ref {
        return Ok(TypeDescriptor::Reference(Reference::Region));
    }

    let std_ref = Datatype::from_descriptor(&TypeDescriptor::Reference(Reference::Std))?;
    if dtype == &std_ref {
        return Ok(TypeDescriptor::Reference(Reference::Std));
    }

    Err(Error::from("Unsupported reference datatype"))
}

fn fallback_type_descriptor(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    match unsafe { H5Tget_class(dtype.id()) } {
        H5T_REFERENCE => detect_reference_descriptor(dtype),
        H5T_VLEN => {
            let super_dtype = unsafe { from_id::<Datatype>(H5Tget_super(dtype.id())) }?;
            Ok(TypeDescriptor::VarLenArray(Box::new(
                type_descriptor_for_dtype(&super_dtype)?,
            )))
        }
        _ => Err(Error::from("Unsupported datatype class")),
    }
}

pub(super) fn type_descriptor_for_dtype(dtype: &Datatype) -> Result<TypeDescriptor, Error> {
    match dtype.to_descriptor() {
        Ok(type_desc) => Ok(type_desc),
        Err(error) if error.to_string() == "Unsupported datatype class" => {
            fallback_type_descriptor(dtype)
        }
        Err(error) => Err(error),
    }
}

pub fn attribute_type_descriptor(attr: &Attribute) -> Result<TypeDescriptor, Error> {
    let dtype = attr.dtype()?;
    type_descriptor_for_dtype(&dtype)
}

pub fn attribute_type_description(attr: &Attribute) -> Result<String, Error> {
    match attribute_type_descriptor(attr) {
        Ok(type_desc) => Ok(type_desc.to_string()),
        Err(error) if error.to_string() == "Unsupported datatype class" => {
            Ok(format!("opaque[{} bytes]", attr.dtype()?.size()))
        }
        Err(error) => Err(error),
    }
}
