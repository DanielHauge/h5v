use std::path::Path;

use hdf5_metno::Attribute;

use crate::{
    h5f::{format_attr_for_edit, HasPath, MetadataRowKind},
    ui::{
        render::AttributeEditable,
        state::{
            AppState, AppToast, AttributeEditRequest,
            AttributeViewSelection::{self, Name, Value},
        },
    },
};

use super::EventResult;

pub(super) fn selected_metadata_row(
    state: &mut AppState<'_>,
) -> Result<(crate::h5f::RenderedAttributeRow, AttributeViewSelection), EventResult> {
    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
    let selection = node.attributes_view_cursor.attribute_view_selection;
    let Some(row_index) = (match node.normalize_attribute_selection() {
        Ok(index) => index,
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    }) else {
        return Err(EventResult::Toast(
            AppToast::Error("No metadata selected".to_string()),
            true,
        ));
    };
    let row = match node.read_attributes() {
        Ok(attributes) => attributes.row(row_index).cloned(),
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    };
    let Some(row) = row else {
        return Err(EventResult::Toast(
            AppToast::Error("No metadata selected".to_string()),
            true,
        ));
    };
    Ok((row, selection))
}

pub(super) fn selected_attribute(
    state: &mut AppState<'_>,
) -> Result<(String, Attribute, AttributeViewSelection), EventResult> {
    let (row, selection) = selected_metadata_row(state)?;
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        let row_name = row.key.unwrap_or_else(|| "selected row".to_string());
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and has no editable HDF5 attribute value",
                row_name
            )),
            false,
        ));
    }
    let attr_name = row.key.unwrap_or_else(|| "selected row".to_string());
    let mut node = state.treeview[state.tree_view_cursor].node.borrow_mut();
    let attributes = match node.read_attributes() {
        Ok(attributes) => attributes,
        Err(error) => {
            return Err(EventResult::Toast(
                AppToast::Error(format!("Failed to read attributes: {}", error)),
                true,
            ))
        }
    };
    let Some((_, attr)) = attributes
        .attributes
        .iter()
        .find(|(name, _)| name == &attr_name)
    else {
        return Err(EventResult::Toast(
            AppToast::Error(format!("Attribute '{}' not found", attr_name)),
            true,
        ));
    };
    Ok((attr_name, attr.clone(), selection))
}

pub(super) fn selected_custom_attribute_name(
    state: &mut AppState<'_>,
) -> Result<String, EventResult> {
    let (row, _) = selected_metadata_row(state)?;
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        let attr_name = row.key.unwrap_or_else(|| "selected row".to_string());
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and cannot be modified",
                attr_name
            )),
            false,
        ));
    }
    row.key.ok_or_else(|| {
        EventResult::Toast(AppToast::Error("No attribute selected".to_string()), true)
    })
}

pub(super) fn selected_attribute_edit_request(
    state: &mut AppState<'_>,
) -> Result<AttributeEditRequest, EventResult> {
    let (row, selection) = selected_metadata_row(state)?;
    let Some(attr_name) = row.key.clone() else {
        return Err(EventResult::Toast(
            AppToast::Error("No attribute selected".to_string()),
            true,
        ));
    };
    if !matches!(row.kind, MetadataRowKind::Attribute) {
        return Err(EventResult::Toast(
            AppToast::Warning(format!(
                "'{}' is a built-in h5v property and cannot be edited",
                attr_name
            )),
            false,
        ));
    }

    let (_, attr, _) = selected_attribute(state)?;
    let edit_name_hint = {
        let node = state.treeview[state.tree_view_cursor].node.borrow();
        let node_path = node.node.path();
        if matches!(selection, Name) || Path::new(&attr_name).extension().is_some() {
            attr_name.clone()
        } else {
            format!("{node_path}/{attr_name}")
        }
    };

    if let Err(error) = attr.can_edit() {
        if let Value = selection {
            return Err(EventResult::Toast(
                AppToast::Error(format!(
                    "Attribute '{}' value cannot be edited: {}",
                    attr_name, error
                )),
                false,
            ));
        }
    }

    let content = match selection {
        Name => attr_name.clone(),
        Value => format_attr_for_edit(&attr)
            .map_err(|error| EventResult::Toast(AppToast::Error(error.to_string()), false))?,
    };

    Ok(AttributeEditRequest {
        attr_name,
        content,
        selection,
        edit_name_hint,
    })
}
