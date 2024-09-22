use std::fmt::{format, Arguments};

use crate::attrs::{AttrMap, FromAttributeRelaxed};
use crate::parser::parse_network;
use crate::plugins::{load_library, NadiExternalPlugin_Ref, NadiPlugin};
use crate::{new_node, AttrSlice, Attribute, FromAttribute, Network, NodeInner};
use crate::{Node, StrPath};
use abi_stable::std_types::{RSlice, RStr, Tuple2};
use abi_stable::{
    external_types::RMutex,
    library::RootModule,
    sabi_trait,
    std_types::{
        RArc, RBox, RErr, RHashMap, ROk,
        ROption::{self, RNone, RSome},
        RResult, RString, RVec,
    },
    StableAbi,
};
use colored::{ColoredString, Colorize};
mod attrs;
mod debug;
mod render;
mod timeseries;

/// Return values for Nadi Functions
#[repr(C)]
#[derive(StableAbi, Default)]
pub enum FunctionRet {
    // a catch rewind system might be nice to catch panic from plugins
    // Panic,
    #[default]
    None,
    Some(Attribute),
    Error(RString),
}

impl FunctionRet {
    pub fn res(self) -> Result<Option<Attribute>, String> {
        match self {
            Self::None => Ok(None),
            Self::Some(a) => Ok(Some(a)),
            Self::Error(e) => Err(e.to_string()),
        }
    }
}

impl From<()> for FunctionRet {
    fn from(value: ()) -> Self {
        Self::None
    }
}

impl<T> From<T> for FunctionRet
where
    Attribute: From<T>,
{
    fn from(value: T) -> Self {
        Self::Some(Attribute::from(value))
    }
}

// Unstable feature where we could define how `?` can be used for generating FunctionRet
// impl<T> std::ops::FromResidual<Result<T, std::io::Error>> for FunctionRet
// where
//     Attribute: From<T>,
// {
//     fn from_residual(residual: Result<T, std::io::Error>) -> Self {
//         match residual {
//             Ok(v) => Self::Some(Attribute::from(v)),
//             Err(e) => Self::Error(RString::from(e.to_string())),
//         }
//     }
// }

impl From<std::io::Error> for FunctionRet {
    fn from(value: std::io::Error) -> Self {
        Self::Error(RString::from(value.to_string()))
    }
}

impl<T> From<Option<T>> for FunctionRet
where
    FunctionRet: From<T>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => Self::from(v),
            None => Self::None,
        }
    }
}

impl<T> From<ROption<T>> for FunctionRet
where
    FunctionRet: From<T>,
{
    fn from(value: ROption<T>) -> Self {
        match value {
            RSome(v) => Self::from(v),
            RNone => Self::None,
        }
    }
}

impl<T, S> From<Result<T, S>> for FunctionRet
where
    FunctionRet: From<T>,
    RString: From<S>,
{
    fn from(value: Result<T, S>) -> Self {
        match value {
            Ok(v) => Self::from(v),
            Err(e) => Self::Error(RString::from(e)),
        }
    }
}

impl<T, S> From<RResult<T, S>> for FunctionRet
where
    FunctionRet: From<T>,
    RString: From<S>,
{
    fn from(value: RResult<T, S>) -> Self {
        match value {
            ROk(v) => Self::from(v),
            RErr(e) => Self::Error(RString::from(e)),
        }
    }
}

#[sabi_trait]
pub trait NodeFunction: Debug {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    fn code(&self) -> RString;
    fn call(&self, obj: &mut NodeInner, ctx: &FunctionCtx) -> FunctionRet;
}

#[sabi_trait]
pub trait NetworkFunction: Debug {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    fn code(&self) -> RString;
    fn call(&self, obj: &mut Network, ctx: &FunctionCtx) -> FunctionRet;
}

// A trait object for the `State` Trait Object
pub type NodeFunctionBox = NodeFunction_TO<'static, RBox<()>>;

pub type NetworkFunctionBox = NetworkFunction_TO<'static, RBox<()>>;

// TODO: add environmental variables, like verbose, progress, debug,
// etc. that all functions can read (passed along with args, kwargs to
// all functions)

// TODO: register functions with plugin name as well, making optional
// plugin.function syntax when disambiguity is required. Can have
// another map with plugin name and functions provided by it. And a
// function to generate html doc with that index.
#[repr(C)]
#[derive(StableAbi, Default)]
pub struct NadiFunctions {
    node: RHashMap<RString, NodeFunctionBox>,
    network: RHashMap<RString, NetworkFunctionBox>,
}

impl NadiFunctions {
    pub fn new() -> Self {
        let mut funcs = Self::default();
        // These things need to be automated if possible, but I don't
        // think that is possible: search all types that implement
        // NadiPlugin trait within functions
        render::RenderMod {}.register(&mut funcs);
        attrs::AttrsMod {}.register(&mut funcs);
        debug::DebugMod {}.register(&mut funcs);
        timeseries::TimeseriesMod {}.register(&mut funcs);
        funcs.load_plugins().unwrap();
        funcs
    }
    pub fn register_network_function(&mut self, func: NetworkFunctionBox) {
        self.network.insert(func.name(), func);
    }
    pub fn register_node_function(&mut self, func: NodeFunctionBox) {
        self.node.insert(func.name(), func);
    }

    pub fn load_plugins(&mut self) -> anyhow::Result<()> {
        for path in std::fs::read_dir("plugins")? {
            let lib = load_library(&path?.path())?;
            println!("Loading: {}", lib.name());
            lib.register(self);
        }
        Ok(())
    }

    pub fn list_functions(&self) {
        println!("Node Functions:");
        for Tuple2(fname, func) in &self.node {
            println!(
                "{fname}\t: {}",
                func.help().split('\n').next().unwrap_or("No Help")
            );
        }
        println!("Network Functions:");
        for Tuple2(fname, func) in &self.network {
            println!(
                "network {fname}\t: {}",
                func.help().split('\n').next().unwrap_or("No Help")
            );
        }
    }

    pub fn execute(&self, func: &FunctionCall, net: &mut Network) -> Result<(), String> {
        match &func.r#type {
            FunctionType::Node(p) => match self.node.get(&func.name) {
                Some(f) => {
                    for node in net.nodes_propagation(p) {
                        // todo manage other return types
                        if let FunctionRet::Error(e) = f.call(&mut node.lock(), func.ctx()) {
                            return Err(e.to_string());
                        }
                    }
                    Ok(())
                }
                None => Err(format!("Node Function {} not found", func.name).into()),
            },
            FunctionType::Network => match self.network.get(&func.name) {
                // todo use returned attribute value
                Some(f) => f.call(net, func.ctx()).res().map(|_| ()),
                None => Err(format!("Network Function {} not found", func.name).into()),
            },
        }
    }

    pub fn help(&self, func: &str) -> Option<String> {
        // node and network function might have same name
        self.help_network(func).or_else(|| self.help_node(func))
    }

    pub fn help_node(&self, func: &str) -> Option<String> {
        self.node
            .get(&RString::from(func))
            .map(|f| f.help().into_string())
    }

    pub fn help_network(&self, func: &str) -> Option<String> {
        self.network
            .get(&RString::from(func))
            .map(|f| f.help().into_string())
    }
    pub fn code(&self, func: &str) -> Option<String> {
        // node and network function might have same name
        self.code_network(func).or_else(|| self.code_node(func))
    }

    pub fn code_node(&self, func: &str) -> Option<String> {
        self.node
            .get(&RString::from(func))
            .map(|f| f.code().into_string())
    }

    pub fn code_network(&self, func: &str) -> Option<String> {
        self.network
            .get(&RString::from(func))
            .map(|f| f.code().into_string())
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, PartialEq)]
pub struct FunctionCall {
    r#type: FunctionType,
    name: RString,
    ctx: FunctionCtx,
}

#[repr(C)]
#[derive(StableAbi, Default, Debug, PartialEq)]
pub struct FunctionCtx {
    args: RVec<Attribute>,
    kwargs: AttrMap,
}

impl FunctionCtx {
    pub fn new(args: Vec<FunctionArg>) -> Self {
        let mut fc = Self::default();
        for l in args {
            match l {
                FunctionArg::Arg(a) => fc.args.push(a),
                FunctionArg::KwArg(KeyVal { key, val }) => {
                    fc.kwargs.insert(key, val);
                }
            }
        }
        fc
    }

    pub fn args(&self) -> AttrSlice {
        self.args.as_rslice()
    }

    pub fn arg(&self, ind: usize) -> Option<&Attribute> {
        self.args.get(ind)
    }

    pub fn kwargs(&self) -> &AttrMap {
        &self.kwargs
    }

    pub fn kwarg(&self, name: &str) -> Option<&Attribute> {
        self.kwargs.get(name)
    }

    pub fn arg_kwarg<P: FromAttribute>(&self, ind: usize, name: &str) -> Option<Result<P, String>> {
        self.kwarg(name).or_else(|| self.arg(ind)).map(|arg| {
            match FromAttribute::try_from_attr(arg) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!(
                    "Argument {} ({} [{}]): {e}",
                    ind + 1,
                    name,
                    nadi_core::attrs::type_name::<P>()
                )),
            }
        })
    }

    pub fn arg_kwarg_relaxed<P: FromAttributeRelaxed>(
        &self,
        ind: usize,
        name: &str,
    ) -> Option<Result<P, String>> {
        self.kwarg(name).or_else(|| self.arg(ind)).map(|arg| {
            match FromAttributeRelaxed::try_from_attr_relaxed(arg) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!(
                    "Argument {} ({} [{}]): {e}",
                    ind + 1,
                    name,
                    nadi_core::attrs::type_name::<P>()
                )),
            }
        })
    }
}

impl FunctionCall {
    pub fn new(r#type: FunctionType, name: &str, args: Option<Vec<FunctionArg>>) -> Self {
        Self {
            r#type,
            name: name.into(),
            ctx: args.map(FunctionCtx::new).unwrap_or_default(),
        }
    }

    pub fn ctx(&self) -> &FunctionCtx {
        &self.ctx
    }

    pub fn to_colored_string(&self) -> String {
        let mut args_str: Vec<String> = self
            .ctx()
            .args()
            .iter()
            .map(|a| Attribute::to_colored_string(a).to_string())
            .collect();
        let kwargs_str: Vec<String> = self
            .ctx()
            .kwargs()
            .iter()
            .map(|Tuple2(k, v)| format!("{}={}", k.to_string().blue(), v.to_colored_string()))
            .collect();
        args_str.extend(kwargs_str.into_iter());
        format!(
            "{} {}({})",
            self.r#type.to_colored_string(),
            self.name.truecolor(80, 80, 200),
            args_str.join(", ")
        )
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum FunctionType {
    Node(Propagation),
    #[default]
    Network,
}

impl ToString for FunctionType {
    fn to_string(&self) -> String {
        match self {
            Self::Node(p) => format!("{}.{}", "node", p.to_string()),
            Self::Network => "network".to_string(),
        }
    }
}

impl FunctionType {
    fn to_colored_string(&self) -> String {
        match self {
            Self::Node(p) => format!("{}.{}", "node".red(), p.to_colored_string()),
            Self::Network => "network".red().to_string(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum Propagation {
    #[default]
    Sequential,
    Inverse,
    InputsFirst,
    OutputFirst,
    List(RVec<RString>),
    Path(StrPath),
}

impl ToString for Propagation {
    fn to_string(&self) -> String {
        match self {
            Self::Sequential => "sequential".to_string(),
            Self::Inverse => "inverse".to_string(),
            Self::InputsFirst => "inputsfirst".to_string(),
            Self::OutputFirst => "outputfirst".to_string(),
            Self::List(v) => v
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            Self::Path(p) => p.to_string(),
        }
    }
}

impl Propagation {
    pub fn to_colored_string(&self) -> String {
        match self {
            Self::Sequential => "sequential".red().to_string(),
            Self::Inverse => "inverse".red().to_string(),
            Self::InputsFirst => "inputsfirst".red().to_string(),
            Self::OutputFirst => "outputfirst".red().to_string(),
            Self::List(v) => v
                .iter()
                .map(|a| a.as_str().green().to_string())
                .collect::<Vec<String>>()
                .join(", "),
            Self::Path(p) => p.to_colored_string(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Clone, PartialEq)]
pub enum FunctionArg {
    Arg(Attribute),
    KwArg(KeyVal),
}

#[repr(C)]
#[derive(StableAbi, Debug, Clone, PartialEq)]
pub struct KeyVal {
    pub key: RString,
    pub val: Attribute,
}