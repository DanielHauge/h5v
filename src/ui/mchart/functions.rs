use std::collections::HashMap;

use super::expression::{self, ExpressionAst};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MchartFunctionCategory {
    Reducer,
    Math,
    Transform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionValueKind {
    Scalar,
    Series,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MchartUnaryMathKind {
    Abs,
    Sqrt,
    Ln,
    Log10,
    Sin,
    Cos,
    Tan,
    Floor,
    Ceil,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MchartReducerKind {
    Mean,
    Min,
    Max,
    Stddev,
    Len,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MchartRollingKind {
    Mean,
    Median,
    Stddev,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MchartScalarCompareKind {
    Max,
    Min,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MchartFunctionExecutionKind {
    Power,
    UnaryMath(MchartUnaryMathKind),
    Reducer(MchartReducerKind),
    Rolling(MchartRollingKind),
    RollingQuantile,
    Threshold,
    Diff,
    ScalarCompare(MchartScalarCompareKind),
    Interp,
    Slice,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MchartFunctionArgDoc {
    pub(crate) name: &'static str,
    pub(crate) kind_label: &'static str,
    pub(crate) detail: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MchartFunctionDefinition {
    pub(crate) name: &'static str,
    pub(crate) category: MchartFunctionCategory,
    pub(crate) summary: &'static str,
    pub(crate) params: &'static [MchartFunctionArgDoc],
    pub(crate) return_label: &'static str,
    pub(crate) example: &'static str,
    pub(crate) completion_insert: &'static str,
    pub(super) execution: MchartFunctionExecutionKind,
    pub(super) top_level_only: bool,
    pub(super) first_arg_direct_item_ref_only: bool,
}

impl MchartFunctionDefinition {
    pub(crate) fn signature(self) -> String {
        let args = self
            .params
            .iter()
            .map(|arg| arg.name)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}({args})", self.name)
    }
}

const SERIES_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "series",
    kind_label: "Series",
    detail: "The input series.",
};

const VALUE_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "value",
    kind_label: "Scalar | Series",
    detail: "Scalar or series to transform.",
};

const WINDOW_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "window",
    kind_label: "Scalar",
    detail: "Window size in samples.",
};

const QUANTILE_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "quantile",
    kind_label: "Scalar",
    detail: "Quantile between 0 and 1.",
};

const THRESHOLD_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "threshold",
    kind_label: "Scalar",
    detail: "Threshold value.",
};

const BASE_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "base",
    kind_label: "Scalar | Series",
    detail: "Base value or series.",
};

const POWER_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "power",
    kind_label: "Scalar | Series",
    detail: "Exponent value or series.",
};

const LHS_SCALAR_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "lhs",
    kind_label: "Scalar",
    detail: "Left scalar value.",
};

const RHS_SCALAR_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "rhs",
    kind_label: "Scalar",
    detail: "Right scalar value.",
};

const XY_SERIES_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "series",
    kind_label: "X/Y Series",
    detail: "Direct chart item reference to an x/y derived series.",
};

const SAMPLE_RATE_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "sample_rate",
    kind_label: "Scalar",
    detail: "Positive resampling step along the x axis.",
};

const START_X_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "start_x",
    kind_label: "Scalar",
    detail: "Inclusive lower x bound.",
};

const END_X_ARG: MchartFunctionArgDoc = MchartFunctionArgDoc {
    name: "end_x",
    kind_label: "Scalar",
    detail: "Inclusive upper x bound.",
};

const MCHART_FUNCTIONS: &[MchartFunctionDefinition] = &[
    MchartFunctionDefinition {
        name: "avg",
        category: MchartFunctionCategory::Reducer,
        summary: "Alias of mean(...); returns the arithmetic mean of the series values.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "avg($1)",
        completion_insert: "avg($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Mean),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "mean",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the arithmetic mean of the series values.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "mean($1)",
        completion_insert: "mean($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Mean),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "min",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the minimum y-value in the series.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "min($1)",
        completion_insert: "min($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Min),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "max",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the maximum y-value in the series.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "max($1)",
        completion_insert: "max($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Max),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "stddev",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the standard deviation of the series values.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "stddev($1)",
        completion_insert: "stddev($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Stddev),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "len",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the number of samples in the series.",
        params: &[SERIES_ARG],
        return_label: "scalar",
        example: "len($1)",
        completion_insert: "len($1)",
        execution: MchartFunctionExecutionKind::Reducer(MchartReducerKind::Len),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "max2",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the larger of two scalar values.",
        params: &[LHS_SCALAR_ARG, RHS_SCALAR_ARG],
        return_label: "scalar",
        example: "max2(mean($1), mean($2))",
        completion_insert: "max2(0.0, 1.0)",
        execution: MchartFunctionExecutionKind::ScalarCompare(MchartScalarCompareKind::Max),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "min2",
        category: MchartFunctionCategory::Reducer,
        summary: "Returns the smaller of two scalar values.",
        params: &[LHS_SCALAR_ARG, RHS_SCALAR_ARG],
        return_label: "scalar",
        example: "min2(max($1), max($2))",
        completion_insert: "min2(0.0, 1.0)",
        execution: MchartFunctionExecutionKind::ScalarCompare(MchartScalarCompareKind::Min),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "abs",
        category: MchartFunctionCategory::Math,
        summary: "Absolute value.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "abs($1)",
        completion_insert: "abs($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Abs),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "sqrt",
        category: MchartFunctionCategory::Math,
        summary: "Square root.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "sqrt(abs($1))",
        completion_insert: "sqrt($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Sqrt),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "ln",
        category: MchartFunctionCategory::Math,
        summary: "Natural logarithm.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "ln($1)",
        completion_insert: "ln($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Ln),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "log10",
        category: MchartFunctionCategory::Math,
        summary: "Base-10 logarithm.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "log10($1)",
        completion_insert: "log10($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Log10),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "sin",
        category: MchartFunctionCategory::Math,
        summary: "Sine.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "sin($1)",
        completion_insert: "sin($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Sin),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "cos",
        category: MchartFunctionCategory::Math,
        summary: "Cosine.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "cos($1)",
        completion_insert: "cos($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Cos),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "tan",
        category: MchartFunctionCategory::Math,
        summary: "Tangent.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "tan($1)",
        completion_insert: "tan($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Tan),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "floor",
        category: MchartFunctionCategory::Math,
        summary: "Round toward negative infinity.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "floor($1)",
        completion_insert: "floor($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Floor),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "ceil",
        category: MchartFunctionCategory::Math,
        summary: "Round toward positive infinity.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "ceil($1)",
        completion_insert: "ceil($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Ceil),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "round",
        category: MchartFunctionCategory::Math,
        summary: "Round to the nearest integer value.",
        params: &[VALUE_ARG],
        return_label: "same shape",
        example: "round($1)",
        completion_insert: "round($1)",
        execution: MchartFunctionExecutionKind::UnaryMath(MchartUnaryMathKind::Round),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "exp",
        category: MchartFunctionCategory::Math,
        summary: "Raises base to power element-wise.",
        params: &[BASE_ARG, POWER_ARG],
        return_label: "same shape",
        example: "exp($1, 2)",
        completion_insert: "exp($1, 2)",
        execution: MchartFunctionExecutionKind::Power,
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_mean",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window mean.",
        params: &[SERIES_ARG, WINDOW_ARG],
        return_label: "series",
        example: "rolling_mean($1, 16)",
        completion_insert: "rolling_mean($1, 16)",
        execution: MchartFunctionExecutionKind::Rolling(MchartRollingKind::Mean),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_median",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window median.",
        params: &[SERIES_ARG, WINDOW_ARG],
        return_label: "series",
        example: "rolling_median($1, 16)",
        completion_insert: "rolling_median($1, 16)",
        execution: MchartFunctionExecutionKind::Rolling(MchartRollingKind::Median),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_stddev",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window standard deviation.",
        params: &[SERIES_ARG, WINDOW_ARG],
        return_label: "series",
        example: "rolling_stddev($1, 16)",
        completion_insert: "rolling_stddev($1, 16)",
        execution: MchartFunctionExecutionKind::Rolling(MchartRollingKind::Stddev),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_min",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window minimum.",
        params: &[SERIES_ARG, WINDOW_ARG],
        return_label: "series",
        example: "rolling_min($1, 16)",
        completion_insert: "rolling_min($1, 16)",
        execution: MchartFunctionExecutionKind::Rolling(MchartRollingKind::Min),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_max",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window maximum.",
        params: &[SERIES_ARG, WINDOW_ARG],
        return_label: "series",
        example: "rolling_max($1, 16)",
        completion_insert: "rolling_max($1, 16)",
        execution: MchartFunctionExecutionKind::Rolling(MchartRollingKind::Max),
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "rolling_quantile",
        category: MchartFunctionCategory::Transform,
        summary: "Sliding-window quantile.",
        params: &[SERIES_ARG, WINDOW_ARG, QUANTILE_ARG],
        return_label: "series",
        example: "rolling_quantile($1, 32, 0.95)",
        completion_insert: "rolling_quantile($1, 32, 0.95)",
        execution: MchartFunctionExecutionKind::RollingQuantile,
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "threshold",
        category: MchartFunctionCategory::Transform,
        summary: "Returns 1.0 where value >= threshold, otherwise 0.0.",
        params: &[VALUE_ARG, THRESHOLD_ARG],
        return_label: "same shape",
        example: "threshold($1, 0.5)",
        completion_insert: "threshold($1, 0.5)",
        execution: MchartFunctionExecutionKind::Threshold,
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "diff",
        category: MchartFunctionCategory::Transform,
        summary: "Discrete first difference of the series.",
        params: &[SERIES_ARG],
        return_label: "series",
        example: "diff($1)",
        completion_insert: "diff($1)",
        execution: MchartFunctionExecutionKind::Diff,
        top_level_only: false,
        first_arg_direct_item_ref_only: false,
    },
    MchartFunctionDefinition {
        name: "interp",
        category: MchartFunctionCategory::Transform,
        summary: "Resample an x/y series onto a uniform x step. Must stay at the top level.",
        params: &[XY_SERIES_ARG, SAMPLE_RATE_ARG],
        return_label: "x/y series",
        example: "interp($1, 0.05)",
        completion_insert: "interp($1, 0.05)",
        execution: MchartFunctionExecutionKind::Interp,
        top_level_only: true,
        first_arg_direct_item_ref_only: true,
    },
    MchartFunctionDefinition {
        name: "slice",
        category: MchartFunctionCategory::Transform,
        summary: "Clip a direct chart item reference to an x-range. Must stay at the top level.",
        params: &[SERIES_ARG, START_X_ARG, END_X_ARG],
        return_label: "series",
        example: "slice($1, 25.5, 250.5)",
        completion_insert: "slice($1, 25.5, 250.5)",
        execution: MchartFunctionExecutionKind::Slice,
        top_level_only: true,
        first_arg_direct_item_ref_only: true,
    },
];

pub(crate) fn mchart_functions() -> &'static [MchartFunctionDefinition] {
    MCHART_FUNCTIONS
}

pub(super) fn find_mchart_function(name: &str) -> Option<&'static MchartFunctionDefinition> {
    MCHART_FUNCTIONS
        .iter()
        .find(|function| function.name.eq_ignore_ascii_case(name))
}

pub(super) fn classify_expression_value_kind(
    expr: &ExpressionAst,
    item_kinds: &HashMap<expression::ExpressionItemRef, ExpressionValueKind>,
    load_kinds: &HashMap<expression::ExpressionLoadRef, ExpressionValueKind>,
) -> Result<ExpressionValueKind, String> {
    match expr {
        ExpressionAst::Number(_) => Ok(ExpressionValueKind::Scalar),
        ExpressionAst::ItemRef(item_ref) => item_kinds
            .get(item_ref)
            .copied()
            .ok_or_else(|| format!("Unknown chart item reference {}", item_ref.render())),
        ExpressionAst::LoadRef(load_ref) => load_kinds
            .get(load_ref)
            .copied()
            .ok_or_else(|| format!("Unknown reference {}", load_ref.render())),
        ExpressionAst::UnaryMinus(inner) => {
            classify_expression_value_kind(inner, item_kinds, load_kinds)
        }
        ExpressionAst::Binary { lhs, rhs, .. } => {
            let lhs = classify_expression_value_kind(lhs, item_kinds, load_kinds)?;
            let rhs = classify_expression_value_kind(rhs, item_kinds, load_kinds)?;
            Ok(
                if lhs == ExpressionValueKind::Series || rhs == ExpressionValueKind::Series {
                    ExpressionValueKind::Series
                } else {
                    ExpressionValueKind::Scalar
                },
            )
        }
        ExpressionAst::FunctionCall { name, args } => {
            classify_registered_function_value_kind(name, args, item_kinds, load_kinds)
        }
    }
}

pub(super) fn classify_registered_function_value_kind(
    name: &str,
    args: &[ExpressionAst],
    item_kinds: &HashMap<expression::ExpressionItemRef, ExpressionValueKind>,
    load_kinds: &HashMap<expression::ExpressionLoadRef, ExpressionValueKind>,
) -> Result<ExpressionValueKind, String> {
    let function =
        find_mchart_function(name).ok_or_else(|| format!("Unsupported function '{name}'"))?;
    let arg_kinds = args
        .iter()
        .map(|arg| classify_expression_value_kind(arg, item_kinds, load_kinds))
        .collect::<Result<Vec<_>, _>>()?;
    match function.execution {
        MchartFunctionExecutionKind::Power => {
            expect_arg_count(function, args, 2)?;
            Ok(if arg_kinds.contains(&ExpressionValueKind::Series) {
                ExpressionValueKind::Series
            } else {
                ExpressionValueKind::Scalar
            })
        }
        MchartFunctionExecutionKind::UnaryMath(_) => {
            expect_arg_count(function, args, 1)?;
            Ok(arg_kinds[0])
        }
        MchartFunctionExecutionKind::Reducer(_) => {
            expect_arg_count(function, args, 1)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series argument")?;
            Ok(ExpressionValueKind::Scalar)
        }
        MchartFunctionExecutionKind::Rolling(_) => {
            expect_arg_count(function, args, 2)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series as the first argument")?;
            expect_scalar_arg(function, 1, arg_kinds[1], "a scalar window argument")?;
            Ok(ExpressionValueKind::Series)
        }
        MchartFunctionExecutionKind::RollingQuantile => {
            expect_arg_count(function, args, 3)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series as the first argument")?;
            expect_scalar_arg(
                function,
                1,
                arg_kinds[1],
                "scalar window and quantile arguments",
            )?;
            expect_scalar_arg(
                function,
                2,
                arg_kinds[2],
                "scalar window and quantile arguments",
            )?;
            Ok(ExpressionValueKind::Series)
        }
        MchartFunctionExecutionKind::Threshold => {
            expect_arg_count(function, args, 2)?;
            expect_scalar_arg(function, 1, arg_kinds[1], "a scalar threshold argument")?;
            Ok(arg_kinds[0])
        }
        MchartFunctionExecutionKind::Diff => {
            expect_arg_count(function, args, 1)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series argument")?;
            Ok(ExpressionValueKind::Series)
        }
        MchartFunctionExecutionKind::ScalarCompare(_) => {
            expect_arg_count(function, args, 2)?;
            expect_scalar_arg(function, 0, arg_kinds[0], "scalar arguments")?;
            expect_scalar_arg(function, 1, arg_kinds[1], "scalar arguments")?;
            Ok(ExpressionValueKind::Scalar)
        }
        MchartFunctionExecutionKind::Interp => {
            expect_arg_count(function, args, 2)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series as the first argument")?;
            expect_scalar_arg(function, 1, arg_kinds[1], "a scalar sample rate argument")?;
            Ok(ExpressionValueKind::Series)
        }
        MchartFunctionExecutionKind::Slice => {
            expect_arg_count(function, args, 3)?;
            expect_series_arg(function, 0, arg_kinds[0], "a series as the first argument")?;
            expect_scalar_arg(function, 1, arg_kinds[1], "finite scalar bounds")?;
            expect_scalar_arg(function, 2, arg_kinds[2], "finite scalar bounds")?;
            Ok(ExpressionValueKind::Series)
        }
    }
}

pub(super) fn interp_call_args(
    ast: &ExpressionAst,
) -> Option<(&expression::ExpressionItemRef, &ExpressionAst)> {
    let ExpressionAst::FunctionCall { name, args } = ast else {
        return None;
    };
    let function = find_mchart_function(name)?;
    if !matches!(function.execution, MchartFunctionExecutionKind::Interp) || args.len() != 2 {
        return None;
    }
    let ExpressionAst::ItemRef(item_ref) = &args[0] else {
        return None;
    };
    Some((item_ref, &args[1]))
}

pub(super) fn slice_call_args(
    ast: &ExpressionAst,
) -> Option<(
    &expression::ExpressionItemRef,
    &ExpressionAst,
    &ExpressionAst,
)> {
    let ExpressionAst::FunctionCall { name, args } = ast else {
        return None;
    };
    let function = find_mchart_function(name)?;
    if !matches!(function.execution, MchartFunctionExecutionKind::Slice) || args.len() != 3 {
        return None;
    }
    let ExpressionAst::ItemRef(item_ref) = &args[0] else {
        return None;
    };
    Some((item_ref, &args[1], &args[2]))
}

pub(super) fn ensure_top_level_transform_usage_is_supported(
    expr: &ExpressionAst,
    allow_top_level: bool,
) -> Result<(), String> {
    match expr {
        ExpressionAst::Number(_) | ExpressionAst::ItemRef(_) | ExpressionAst::LoadRef(_) => Ok(()),
        ExpressionAst::UnaryMinus(inner) => {
            ensure_top_level_transform_usage_is_supported(inner, false)
        }
        ExpressionAst::Binary { lhs, rhs, .. } => {
            ensure_top_level_transform_usage_is_supported(lhs, false)?;
            ensure_top_level_transform_usage_is_supported(rhs, false)
        }
        ExpressionAst::FunctionCall { name, args } => {
            if let Some(function) = find_mchart_function(name) {
                if function.top_level_only {
                    if !allow_top_level {
                        return Err(format!(
                            "{}() must be the top-level expression",
                            function.name
                        ));
                    }
                    expect_arg_count(function, args, function.params.len())?;
                    if function.first_arg_direct_item_ref_only
                        && !matches!(args.first(), Some(ExpressionAst::ItemRef(_)))
                    {
                        return Err(format!(
                            "{}() requires a direct chart item reference as the first argument",
                            function.name
                        ));
                    }
                }
            }
            for arg in args {
                ensure_top_level_transform_usage_is_supported(arg, false)?;
            }
            Ok(())
        }
    }
}

fn expect_arg_count(
    function: &MchartFunctionDefinition,
    args: &[ExpressionAst],
    expected: usize,
) -> Result<(), String> {
    if args.len() != expected {
        return Err(format!(
            "{}() expects exactly {expected} arguments",
            function.name
        ));
    }
    Ok(())
}

fn expect_series_arg(
    function: &MchartFunctionDefinition,
    _index: usize,
    kind: ExpressionValueKind,
    message: &str,
) -> Result<(), String> {
    if kind != ExpressionValueKind::Series {
        return Err(format!("{}() requires {message}", function.name));
    }
    Ok(())
}

fn expect_scalar_arg(
    function: &MchartFunctionDefinition,
    _index: usize,
    kind: ExpressionValueKind,
    message: &str,
) -> Result<(), String> {
    if kind != ExpressionValueKind::Scalar {
        return Err(format!("{}() requires {message}", function.name));
    }
    Ok(())
}
