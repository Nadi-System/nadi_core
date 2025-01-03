use abi_stable::std_types::{RString, RVec, Tuple2};
use std::collections::HashMap;
use crate::functions::Propagation;
use crate::functions::NadiFunctions;
use colored::Colorize;
use crate::prelude::*;

pub struct TaskContext {
    net: Network,
    functions: NadiFunctions,
    env: AttrMap,
}

impl TaskContext {
    pub fn execute(&mut self, task: Task) -> Result<String, String> {
        match &task.ty {
	    TaskType::Exit => std::process::exit(0),
	    TaskType::Env => {
		if let Some(var) = task.attribute {
		    match task.input {
			TaskInput::Literal(val) => {
			    self.env.insert(var.into(), val);
			    Ok("".into())
			},
			TaskInput::None => {
			    if let Some(v) = self.env.get(var.as_str()) {
				Ok(v.to_colored_string())
			    } else {
				Err(String::from("Env variable {var} doesn't exist"))
			    }
			}
			_ => Err(String::from("Couldn't set env variable"))
		    }
		} else {
		    let mut list = String::new();
		    for Tuple2(k, v) in &self.env {
			list.push_str(&format!("{k}={}", v.to_colored_string()));
		    }
		    Ok(list)
		}
	    },
            // TaskType::Node(p) => {
	    // 	let prop = self.net.nodes_propagation(p);
	    // 	match task.input {
	    // 	    TaskInput::None => {
	    // 		if let Some(attr) = task.attribute {
	    // 		    for n in prop {
	    // 			let n = n.lock();
	    // 			println!("{}: {}", n.name(), n.attr(&attr));
	    // 		    }
	    // 		}
	    // 	    }
	    // 	    TaskInput::Literal(a) => {
	    // 		if let Some(attr) = task.attribute {
	    // 		    for n in prop {
	    // 			let n = n.lock();
	    // 			n.set_attr(&attr, a.clone());
	    // 		    }
	    // 		}
	    // 	    }
	    // 	    TaskInput::Variable(var) => {
	    // 		if let Some(attr) = task.attribute {
	    // 		    for n in prop {
	    // 			let n = n.lock();
	    // 			// todo if let Some
	    // 			if let Some(v) = n.attr(&var) {
	    // 			    n.set_attr(&attr, v.clone());
	    // 			} else {
	    // 			    todo!()
	    // 			}
	    // 		    }
	    // 		}
	    // 	    }
	    // 	    TaskInput::Function(fc) => {			
	    // 		match self.node(&fc.name) {
	    // 		    Some(f) => {
	    // 			let p: RVec<Node> = prop.into();
	    // 			// todo manage other return types
	    // 			f.call(p.as_rslice(), fc.args, fc.kwargs, &self.env)
	    // 			    .map_err(|e| e.to_string())
	    // 			    .into()
	    // 		    }
	    // 		    None => Err(format!("Node Function {} not found", func.name)),
	    // 		}
	    // 	    }
	    // 	}},
            // TaskType::Network => match self.network(&func.name) {
            //     // todo use returned attribute value
            //     Some(f) => f.call(net, func.ctx()).map_err(|e| e.to_string()).into(),
            //     None => Err(format!("Network Function {} not found", func.name)),
            // },
            TaskType::Help(None, Some(var)) => {
                let mut helpstr = String::new();
                if let Some(help) = self.functions.node(&var).map(|f| f.help()) {
                    helpstr = format!("{}: {}\n{}", "== Node Function".blue(), var, help);
                }
                if let Some(help) = self.functions.network(&var).map(|f| f.help()) {
                    helpstr.push_str(&format!("\n{}: {}\n{}", "== Network Function".blue(), var, help));
                }
                if !helpstr.is_empty() {
                    Ok(helpstr)
                } else {
                    Err(format!("Function {} not found", var))
                }
            }
            TaskType::Help(Some(TaskKeyword::Node), Some(var)) => {
                if let Some(help) = self.functions.node(&var).map(|f| f.help()) {
                    Ok(format!("{}: {}\n{}", "== Node Function".blue(), var, help))
                    
                } else {
                    Err(format!("Node Function {} not found", var))
                }
            }
            TaskType::Help(Some(TaskKeyword::Network), Some(var)) => {
                if let Some(help) = self.functions.network(&var).map(|f| f.help()) {
                    Ok(format!("{}: {}\n{}", "== Network Function".blue(), var, help))
                } else {
                    Err(format!("Network Function {} not found", var))
                }
            }
            TaskType::Help(Some(TaskKeyword::Env), None) => {
		Ok(format!("Set Environmental Variable"))
	    }
	    _ => todo!()
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Task {
    pub ty: TaskType,
    pub attribute: Option<String>,
    pub input: TaskInput,
}

impl Task {
    pub fn to_colored_string(&self) -> String {
	if let Some(ref a) = self.attribute {
            format!("{}.{} = {}", self.ty.to_colored_string(), a.green(), self.input.to_colored_string())
	}else {
            format!("{} {}", self.ty.to_colored_string(), self.input.to_colored_string())
	}
    }

    pub fn exit() -> Self {
        Task {
            ty: TaskType::Exit,
            attribute: None,
            input: TaskInput::None,
        }
    }
    
    pub fn env(var: String, val: Attribute) -> Self {
        Task {
            ty: TaskType::Env,
            attribute: Some(var),
            input: TaskInput::Literal(val),
        }
    }

    pub fn help(kw: Option<TaskKeyword>, var: Option<String>) -> Self {
        Task {
            ty: TaskType::Help(kw, var),
            attribute: None,
            input: TaskInput::None,
        }
    }
}


#[derive(Clone, PartialEq, Debug)]
pub enum TaskType {
    Node(Propagation),
    Network,
    Env,
    Help(Option<TaskKeyword>, Option<String>),
    Exit,
}

impl ToString for TaskType {
    fn to_string(&self) -> String {
        match self {
            Self::Node(p) => format!("node{}", p.to_string()),
            Self::Network => "network".to_string(),
            Self::Help(None, None) => "help".to_string(),
            Self::Help(Some(x), None) => format!("help {}", x.to_string()),
            Self::Help(None, Some(y)) => format!("help {}", y),
            Self::Help(Some(x), Some(y)) => format!("help {} {}", x.to_string(), y),
            Self::Env => "env".to_string(),
	    Self::Exit => "exit".to_string(),
        }
    }
}

impl TaskType {
    fn to_colored_string(&self) -> String {
        match self {
            Self::Node(p) => format!("{}{}", "node".red(), p.to_string()),
            Self::Network => "network".red().to_string(),
            Self::Help(None, None) => "help".red().to_string(),
            Self::Help(Some(x), None) => format!("{} {}", "help".red(), x.to_string().red()),
            Self::Help(None, Some(y)) => format!("{} {}", "help".red(), y),
            Self::Help(Some(x), Some(y)) => format!("{} {} {}", "help".red(), x.to_string().red(), y),
            Self::Env => "env".red().to_string(),
	    Self::Exit => "exit".red().to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskInput {
    None,
    Function(FunctionCall),
    Literal(Attribute),
    Variable(String),
}

impl TaskInput {
    pub fn to_colored_string(&self) -> String {
	match self {
	    Self::None => "".into(),
	    Self::Function(fc) => fc.to_colored_string(),
	    Self::Literal(a) => a.to_colored_string(),
	    Self::Variable(s) => s.green().to_string(),
	}
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<TaskInput>,
    pub kwargs: HashMap<String, TaskInput>,
}

impl FunctionCall {
    pub fn to_colored_string(&self) -> String {
        let mut args_str: Vec<String> = self
            .args
            .iter()
            .map(|a| a.to_colored_string().to_string())
            .collect();
        let kwargs_str: Vec<String> = self
            .kwargs
            .iter()
            .map(|(k, v)| format!("{}={}", k.to_string().blue(), v.to_colored_string()))
            .collect();
        args_str.extend(kwargs_str);
        format!(
            "{}({})",
            self.name.truecolor(80, 80, 200),
            args_str.join(", ")
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskKeyword {
    Node,
    Network,
    Env,
    Exit,
    Help,
}

impl ToString for TaskKeyword {
    fn to_string(&self) -> String {
        match self {
	    TaskKeyword::Node => "node",
	    TaskKeyword::Network => "network",
	    TaskKeyword::Env => "env",
	    TaskKeyword::Exit => "exit",
	    TaskKeyword::Help => "help",
	}.to_string()
    }
}
