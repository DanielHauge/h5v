use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, Type};

// ─── type helpers ─────────────────────────────────────────────────────────────

fn is_color(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "Color").unwrap_or(false))
}

fn array_color_len(ty: &Type) -> Option<usize> {
    if let Type::Array(arr) = ty {
        if is_color(&arr.elem) {
            if let syn::Expr::Lit(el) = &arr.len {
                if let syn::Lit::Int(li) = &el.lit {
                    return li.base10_parse().ok();
                }
            }
        }
    }
    None
}

fn is_symbol(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Reference(tr)
            if matches!(tr.elem.as_ref(), Type::Path(tp)
                if tp.path.segments.last().map(|s| s.ident == "str").unwrap_or(false))
    )
}

fn array_symbol_len(ty: &Type) -> Option<usize> {
    if let Type::Array(arr) = ty {
        if is_symbol(&arr.elem) {
            if let syn::Expr::Lit(el) = &arr.len {
                if let syn::Lit::Int(li) = &el.lit {
                    return li.base10_parse().ok();
                }
            }
        }
    }
    None
}

fn derive_error(ast: &DeriveInput, message: &str) -> TokenStream {
    syn::Error::new_spanned(ast, message)
        .to_compile_error()
        .into()
}

fn field_ident<'a>(field: &'a Field, derive_name: &str) -> Result<&'a syn::Ident, syn::Error> {
    field.ident.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(field, format!("{derive_name} requires named fields"))
    })
}

// ─── #[derive(ColorGroup)] ────────────────────────────────────────────────────

#[proc_macro_derive(ColorGroup)]
pub fn derive_color_group(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return derive_error(&ast, "ColorGroup requires named fields"),
        },
        _ => return derive_error(&ast, "ColorGroup requires a struct"),
    };

    // For each field, emit code blocks for each of the four generated methods.
    // `group` is a runtime &str, so prefix-stripping happens at runtime —
    // the struct itself doesn't know which field name it has in ThemeColors.
    let mut entries_stmts = Vec::<TokenStream2>::new();
    let mut get_stmts = Vec::<TokenStream2>::new();
    let mut set_stmts = Vec::<TokenStream2>::new();
    let mut names_stmts = Vec::<TokenStream2>::new();

    for field in fields {
        let ident = match field_ident(field, "ColorGroup") {
            Ok(ident) => ident,
            Err(error) => return error.to_compile_error().into(),
        };
        let raw = ident.to_string(); // e.g. "chart_axis", "title"
        let ty = &field.ty;

        if is_color(ty) {
            // ── plain Color ──────────────────────────────────────────────────
            // Runtime key: if raw starts with "{group}_", strip that prefix.
            entries_stmts.push(quote! {
                {
                    let key = Self::lua_key(group, #raw);
                    out.push((key, self.#ident));
                }
            });
            get_stmts.push(quote! {
                {
                    let suffix = Self::key_suffix(group, #raw);
                    if key == suffix || key == #raw {
                        return Some(self.#ident);
                    }
                }
            });
            set_stmts.push(quote! {
                {
                    let suffix = Self::key_suffix(group, #raw);
                    if key == suffix || key == #raw {
                        self.#ident = color;
                        return true;
                    }
                }
            });
            names_stmts.push(quote! {
                out.push(Self::lua_key(group, #raw));
            });
        } else if let Some(n) = array_color_len(ty) {
            // ── [Color; N] ───────────────────────────────────────────────────
            // Expands to "{base}_1" .. "{base}_N" where base = key_suffix of the field.
            let indices: Vec<usize> = (0..n).collect();
            let one_idx: Vec<usize> = (1..=n).collect();

            entries_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        {
                            let key = Self::static_str(format!("{}.{}_{}", group, base, #one_idx));
                            out.push((key, self.#ident[#indices]));
                        }
                    )*
                }
            });
            get_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        if key == format!("{}_{}", base, #one_idx).as_str() {
                            return Some(self.#ident[#indices]);
                        }
                    )*
                }
            });
            set_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        if key == format!("{}_{}", base, #one_idx).as_str() {
                            self.#ident[#indices] = color;
                            return true;
                        }
                    )*
                }
            });
            names_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        out.push(Self::static_str(format!("{}.{}_{}", group, base, #one_idx)));
                    )*
                }
            });
        } else {
            panic!(
                "ColorGroup: `{}::{}` is neither Color nor [Color; N]",
                name, raw
            );
        }
    }

    quote! {
        impl #name {
            // ── internal helpers ──────────────────────────────────────────────

            /// Strip the `{group}_` prefix from a raw field name, returning
            /// the suffix that becomes the Lua key. E.g. ("chart", "chart_axis") → "axis".
            /// If the field doesn't start with the prefix, return it unchanged.
            fn key_suffix<'a>(group: &str, raw: &'a str) -> &'a str {
                let prefix_len = group.len() + 1; // "chart_".len()
                if raw.len() > prefix_len && raw.starts_with(group) && raw.as_bytes()[group.len()] == b'_' {
                    &raw[prefix_len..]
                } else {
                    raw
                }
            }

            /// Build a dotted key string and leak it into `'static`.
            /// Only called during config init / scaffolding — never in render loops.
            fn lua_key(group: &str, raw: &str) -> &'static str {
                let suffix = Self::key_suffix(group, raw);
                Self::static_str(format!("{}.{}", group, suffix))
            }

            fn static_str(s: String) -> &'static str {
                Box::leak(s.into_boxed_str())
            }

            // ── public generated API ──────────────────────────────────────────

            /// All (dotted_key, Color) pairs for this group.
            pub(crate) fn color_entries(&self, group: &str) -> Vec<(&'static str, ratatui::prelude::Color)> {
                let mut out = Vec::new();
                #( #entries_stmts )*
                out
            }

            /// Look up a color by the key *after* the dot.
            /// Accepts both the stripped suffix ("axis") and the raw field name ("chart_axis").
            pub(crate) fn get_color(&self, group: &str, key: &str) -> Option<ratatui::prelude::Color> {
                #( #get_stmts )*
                None
            }

            /// Set a color by the key after the dot. Returns false if not found.
            pub(crate) fn set_color(&mut self, group: &str, key: &str, color: ratatui::prelude::Color) -> bool {
                #( #set_stmts )*
                false
            }

            /// All dotted key strings for this group, e.g. `["chart.axis", "chart.series_1", …]`.
            pub(crate) fn key_names(group: &str) -> Vec<&'static str> {
                let mut out: Vec<&'static str> = Vec::new();
                #( #names_stmts )*
                out
            }
        }
    }
    .into()
}

// ─── #[derive(SymbolGroup)] ───────────────────────────────────────────────────

#[proc_macro_derive(SymbolGroup)]
pub fn derive_symbol_group(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return derive_error(&ast, "SymbolGroup requires named fields"),
        },
        _ => return derive_error(&ast, "SymbolGroup requires a struct"),
    };

    let mut entries_stmts = Vec::<TokenStream2>::new();
    let mut get_stmts = Vec::<TokenStream2>::new();
    let mut set_stmts = Vec::<TokenStream2>::new();
    let mut names_stmts = Vec::<TokenStream2>::new();

    for field in fields {
        let ident = match field_ident(field, "SymbolGroup") {
            Ok(ident) => ident,
            Err(error) => return error.to_compile_error().into(),
        };
        let raw = ident.to_string();
        let ty = &field.ty;

        if is_symbol(ty) {
            entries_stmts.push(quote! {
                {
                    let key = Self::lua_key(group, #raw);
                    out.push((key, self.#ident));
                }
            });
            get_stmts.push(quote! {
                {
                    let suffix = Self::key_suffix(group, #raw);
                    if key == suffix || key == #raw {
                        return Some(self.#ident);
                    }
                }
            });
            set_stmts.push(quote! {
                {
                    let suffix = Self::key_suffix(group, #raw);
                    if key == suffix || key == #raw {
                        self.#ident = Self::static_str(value.to_string());
                        return true;
                    }
                }
            });
            names_stmts.push(quote! {
                out.push(Self::lua_key(group, #raw));
            });
        } else if let Some(n) = array_symbol_len(ty) {
            let indices: Vec<usize> = (0..n).collect();
            let one_idx: Vec<usize> = (1..=n).collect();

            entries_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        {
                            let key = Self::static_str(format!("{}.{}_{}", group, base, #one_idx));
                            out.push((key, self.#ident[#indices]));
                        }
                    )*
                }
            });
            get_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        if key == format!("{}_{}", base, #one_idx).as_str() {
                            return Some(self.#ident[#indices]);
                        }
                    )*
                }
            });
            set_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        if key == format!("{}_{}", base, #one_idx).as_str() {
                            self.#ident[#indices] = Self::static_str(value.to_string());
                            return true;
                        }
                    )*
                }
            });
            names_stmts.push(quote! {
                {
                    let base = Self::key_suffix(group, #raw);
                    #(
                        out.push(Self::static_str(format!("{}.{}_{}", group, base, #one_idx)));
                    )*
                }
            });
        } else {
            panic!(
                "SymbolGroup: `{}::{}` is neither &'static str nor [&'static str; N]",
                name, raw
            );
        }
    }

    quote! {
        impl #name {
            fn key_suffix<'a>(group: &str, raw: &'a str) -> &'a str {
                let prefix_len = group.len() + 1;
                if raw.len() > prefix_len && raw.starts_with(group) && raw.as_bytes()[group.len()] == b'_' {
                    &raw[prefix_len..]
                } else {
                    raw
                }
            }

            fn lua_key(group: &str, raw: &str) -> &'static str {
                let suffix = Self::key_suffix(group, raw);
                Self::static_str(format!("{}.{}", group, suffix))
            }

            fn static_str(s: String) -> &'static str {
                Box::leak(s.into_boxed_str())
            }

            pub(crate) fn symbol_entries(&self, group: &str) -> Vec<(&'static str, &'static str)> {
                let mut out = Vec::new();
                #( #entries_stmts )*
                out
            }

            pub(crate) fn get_symbol(&self, group: &str, key: &str) -> Option<&'static str> {
                #( #get_stmts )*
                None
            }

            pub(crate) fn set_symbol(&mut self, group: &str, key: &str, value: &str) -> bool {
                #( #set_stmts )*
                false
            }

            pub(crate) fn key_names(group: &str) -> Vec<&'static str> {
                let mut out: Vec<&'static str> = Vec::new();
                #( #names_stmts )*
                out
            }
        }
    }
    .into()
}

// ─── #[derive(ThemeColorCatalog)] ────────────────────────────────────────────

#[proc_macro_derive(ThemeColorCatalog)]
pub fn derive_theme_color_catalog(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return derive_error(&ast, "ThemeColorCatalog requires named fields"),
        },
        _ => return derive_error(&ast, "ThemeColorCatalog requires a struct"),
    };

    // Collect (ident, ty, group_name_string) for each sub-struct field.
    let groups = match fields
        .iter()
        .map(|field| {
            let ident = field_ident(field, "ThemeColorCatalog")?.clone();
            Ok((ident.clone(), field.ty.clone(), ident.to_string()))
        })
        .collect::<Result<Vec<(syn::Ident, syn::Type, String)>, syn::Error>>()
    {
        Ok(groups) => groups,
        Err(error) => return error.to_compile_error().into(),
    };

    // named_color: match "group.*" arms, then bare-key fallback
    let named_arms: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                k if k.starts_with(concat!(#g, ".")) => {
                    self.#id.get_color(#g, &k[#g.len() + 1..])
                }
            }
        })
        .collect();

    // set_named_color: same structure
    let set_arms: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                k if k.starts_with(concat!(#g, ".")) => {
                    self.#id.set_color(#g, &k[#g.len() + 1..], color)
                }
            }
        })
        .collect();

    // Fallback linear scan for legacy bare keys ("title", "bg", etc.)
    let fallback_get: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                if let Some(c) = self.#id.get_color(#g, bare) { return Some(c); }
            }
        })
        .collect();

    let fallback_set: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                if self.#id.set_color(#g, bare, color) { return true; }
            }
        })
        .collect();

    // all_color_names / all_color_entries: just call each sub-struct
    let names_stmts: Vec<TokenStream2> = groups
        .iter()
        .map(|(_, ty, g)| {
            quote! {
                out.extend(#ty::key_names(#g));
            }
        })
        .collect();

    let entries_stmts: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                out.extend(self.#id.color_entries(#g));
            }
        })
        .collect();

    quote! {
        impl #name {
            /// Get a color by fully-qualified name, e.g. `"text.title"`, `"chart.series_1"`.
            /// Also accepts legacy bare keys like `"title"` via a linear fallback scan.
            pub(crate) fn named_color(&self, name: &str) -> Option<ratatui::prelude::Color> {
                let norm = super::catalog::normalize_color_name(name);
                let k    = norm.as_str();
                match k {
                    #( #named_arms )*
                    bare => {
                        #( #fallback_get )*
                        None
                    }
                }
            }

            /// Set a color by fully-qualified name. Returns false if the name is unknown.
            pub(crate) fn set_named_color(&mut self, name: &str, color: ratatui::prelude::Color) -> bool {
                let norm = super::catalog::normalize_color_name(name);
                let k    = norm.as_str();
                match k {
                    #( #set_arms )*
                    bare => {
                        #( #fallback_set )*
                        false
                    }
                }
            }

            /// Every valid dotted color name, e.g. `["text.title", "chart.series_1", …]`.
            /// Replaces the hand-written `COLOR_NAMES` constant and `available_color_names()`.
            pub(crate) fn all_color_names(&self) -> Vec<&'static str> {
                let mut out: Vec<&'static str> = Vec::new();
                #( #names_stmts )*
                out
            }

            /// All (dotted_name, color) pairs for the current theme values.
            /// Used by `theme_named_colors()` and `build_theme_table()`.
            pub(crate) fn all_color_entries(&self) -> Vec<(&'static str, ratatui::prelude::Color)> {
                let mut out = Vec::new();
                #( #entries_stmts )*
                out
            }
        }
    }
    .into()
}

// ─── #[derive(ThemeSymbolCatalog)] ───────────────────────────────────────────

#[proc_macro_derive(ThemeSymbolCatalog)]
pub fn derive_theme_symbol_catalog(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(f) => &f.named,
            _ => return derive_error(&ast, "ThemeSymbolCatalog requires named fields"),
        },
        _ => return derive_error(&ast, "ThemeSymbolCatalog requires a struct"),
    };

    let groups = match fields
        .iter()
        .map(|field| {
            let ident = field_ident(field, "ThemeSymbolCatalog")?.clone();
            Ok((ident.clone(), field.ty.clone(), ident.to_string()))
        })
        .collect::<Result<Vec<(syn::Ident, syn::Type, String)>, syn::Error>>()
    {
        Ok(groups) => groups,
        Err(error) => return error.to_compile_error().into(),
    };

    let named_arms: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                k if k.starts_with(concat!(#g, ".")) => {
                    self.#id.get_symbol(#g, &k[#g.len() + 1..])
                }
            }
        })
        .collect();

    let set_arms: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                k if k.starts_with(concat!(#g, ".")) => {
                    self.#id.set_symbol(#g, &k[#g.len() + 1..], value)
                }
            }
        })
        .collect();

    let fallback_get: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                if let Some(symbol) = self.#id.get_symbol(#g, bare) { return Some(symbol); }
            }
        })
        .collect();

    let fallback_set: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                if self.#id.set_symbol(#g, bare, value) { return true; }
            }
        })
        .collect();

    let names_stmts: Vec<TokenStream2> = groups
        .iter()
        .map(|(_, ty, g)| {
            quote! {
                out.extend(#ty::key_names(#g));
            }
        })
        .collect();

    let entries_stmts: Vec<TokenStream2> = groups
        .iter()
        .map(|(id, _, g)| {
            quote! {
                out.extend(self.#id.symbol_entries(#g));
            }
        })
        .collect();

    quote! {
        impl #name {
            pub(crate) fn named_symbol(&self, name: &str) -> Option<&'static str> {
                let norm = super::catalog::normalize_symbol_name(name);
                let k = norm.as_str();
                match k {
                    #( #named_arms )*
                    bare => {
                        #( #fallback_get )*
                        None
                    }
                }
            }

            pub(crate) fn set_named_symbol(&mut self, name: &str, value: &str) -> bool {
                let norm = super::catalog::normalize_symbol_name(name);
                let k = norm.as_str();
                match k {
                    #( #set_arms )*
                    bare => {
                        #( #fallback_set )*
                        false
                    }
                }
            }

            pub(crate) fn all_symbol_names(&self) -> Vec<&'static str> {
                let mut out: Vec<&'static str> = Vec::new();
                #( #names_stmts )*
                out
            }

            pub(crate) fn all_symbol_entries(&self) -> Vec<(&'static str, &'static str)> {
                let mut out = Vec::new();
                #( #entries_stmts )*
                out
            }
        }
    }
    .into()
}
