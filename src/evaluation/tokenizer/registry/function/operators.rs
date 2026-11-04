use std::sync::Arc;

use crate::{
    AbacusError, AngleMode, TimeZone, UnitRegistry, Value, WeekendDays,
    units::eval_result::EvalResult,
};

/// Engine settings visible to a function while it is evaluated (see [`FunctionOp::with_context`]).
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct FunctionContext<'a> {
    /// Angle unit assumed for bare numbers (`sin(90)` in degree mode).
    pub angle_mode: AngleMode,
    /// Weekend days for business-day calculations.
    pub weekend: WeekendDays,
    /// Timezone for dates written without one.
    pub default_timezone: Option<&'a TimeZone>,
    /// The engine's unit registry.
    pub units: &'a UnitRegistry,
}

impl<'a> FunctionContext<'a> {
    /// A context with default settings (radians, Saturday/Sunday weekend, no timezone) over
    /// `units`.
    #[must_use]
    pub fn new(units: &'a UnitRegistry) -> Self {
        Self {
            angle_mode: AngleMode::default(),
            weekend: WeekendDays::default(),
            default_timezone: None,
            units,
        }
    }

    /// Sets the angle mode.
    #[must_use]
    pub fn with_angle_mode(mut self, angle_mode: AngleMode) -> Self {
        self.angle_mode = angle_mode;
        self
    }

    /// Sets the weekend days.
    #[must_use]
    pub fn with_weekend(mut self, weekend: WeekendDays) -> Self {
        self.weekend = weekend;
        self
    }

    /// Sets the default timezone.
    #[must_use]
    pub fn with_default_timezone(mut self, tz: Option<&'a TimeZone>) -> Self {
        self.default_timezone = tz;
        self
    }
}

/// Signature of a function that sees the engine settings.
pub type ContextFn =
    dyn Fn(&FunctionContext<'_>, &[EvalResult]) -> Result<EvalResult, AbacusError> + Send + Sync;

#[derive(Clone)]
pub enum FunctionTarget {
    Scalar(fn(&[Value]) -> Result<Value, AbacusError>),
    EvalResult(fn(&[Value]) -> Result<EvalResult, AbacusError>),
    General(fn(&[EvalResult]) -> Result<EvalResult, AbacusError>),
    /// A function or closure that also receives the engine settings.
    WithContext(Arc<ContextFn>),
}

impl std::fmt::Debug for FunctionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::Scalar(_) => "Scalar",
            Self::EvalResult(_) => "EvalResult",
            Self::General(_) => "General",
            Self::WithContext(_) => "WithContext",
        };
        f.write_str(kind)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionOp {
    pub name: &'static str,
    pub min_args: usize,
    pub max_args: usize,
    pub func: FunctionTarget,
}

/// Standard units for [`FunctionOp::apply`], which has no engine to take them from.
fn default_units() -> &'static UnitRegistry {
    static UNITS: std::sync::LazyLock<UnitRegistry> =
        std::sync::LazyLock::new(UnitRegistry::standard);
    &UNITS
}

fn scalar_args(raw_args: &[EvalResult]) -> Result<Vec<Value>, AbacusError> {
    raw_args
        .iter()
        .map(|arg| arg.as_scalar().cloned())
        .collect()
}

impl FunctionOp {
    pub fn scalar(
        name: &'static str,
        min_args: usize,
        max_args: usize,
        f: fn(&[Value]) -> Result<Value, AbacusError>,
    ) -> Self {
        Self {
            name,
            min_args,
            max_args,
            func: FunctionTarget::Scalar(f),
        }
    }

    pub fn eval_result(
        name: &'static str,
        min_args: usize,
        max_args: usize,
        f: fn(&[Value]) -> Result<EvalResult, AbacusError>,
    ) -> Self {
        Self {
            name,
            min_args,
            max_args,
            func: FunctionTarget::EvalResult(f),
        }
    }

    pub fn general(
        name: &'static str,
        min_args: usize,
        max_args: usize,
        f: fn(&[EvalResult]) -> Result<EvalResult, AbacusError>,
    ) -> Self {
        Self {
            name,
            min_args,
            max_args,
            func: FunctionTarget::General(f),
        }
    }

    /// A function that receives the engine settings. `f` may be a closure capturing state.
    ///
    /// ```rust
    /// use abacus::{Abacus, EvalResult, FunctionOp, Value};
    ///
    /// let rate = 1.5;
    /// let mut calc = Abacus::standard();
    /// calc.register_function_token(FunctionOp::with_context("boost", 1, 1, move |_ctx, args| {
    ///     let x = args[0].as_scalar()?;
    ///     Ok(EvalResult::Scalar(Value::dimensionless(x.canonical * rate)))
    /// }));
    /// assert_eq!(calc.eval("boost(4)").unwrap().to_display(), "6");
    /// ```
    pub fn with_context(
        name: &'static str,
        min_args: usize,
        max_args: usize,
        f: impl Fn(&FunctionContext<'_>, &[EvalResult]) -> Result<EvalResult, AbacusError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            name,
            min_args,
            max_args,
            func: FunctionTarget::WithContext(Arc::new(f)),
        }
    }

    /// Applies the function with default settings (see [`FunctionContext::new`]).
    pub fn apply(&self, raw_args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
        self.apply_with(&FunctionContext::new(default_units()), raw_args)
    }

    /// Applies the function with the given engine settings.
    pub fn apply_with(
        &self,
        ctx: &FunctionContext<'_>,
        raw_args: &[EvalResult],
    ) -> Result<EvalResult, AbacusError> {
        if raw_args.len() < self.min_args || raw_args.len() > self.max_args {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        match &self.func {
            FunctionTarget::General(f) => f(raw_args),
            FunctionTarget::Scalar(f) => f(&scalar_args(raw_args)?).map(EvalResult::Scalar),
            FunctionTarget::EvalResult(f) => f(&scalar_args(raw_args)?),
            FunctionTarget::WithContext(f) => f(ctx, raw_args),
        }
    }

    pub fn apply_scalar(&self, args: &[Value]) -> Result<EvalResult, AbacusError> {
        let eval_args: Vec<EvalResult> = args.iter().cloned().map(EvalResult::Scalar).collect();
        self.apply(&eval_args)
    }
}
