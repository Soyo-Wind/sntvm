use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    io::{self, Write},
    sync::Arc,
    env,
    fs
};

// ===== Float wrapper =====
#[derive(Clone, Copy, Debug)]
struct Float(f64);

impl PartialEq for Float {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Eq for Float {}
impl Hash for Float {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state)
    }
}

// ===== Value =====
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Int(i32),
    Float(Float),
    Bool(bool),
    Str(Arc<String>),
    List(Arc<Vec<Value>>),
    Set(Arc<HashSet<Value>>),
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Int(i) => i.hash(state),
            Value::Float(f) => f.hash(state),
            Value::Bool(b) => b.hash(state),
            Value::Str(s) => s.hash(state),
            Value::List(v) => {
                for e in v.iter() {
                    e.hash(state);
                }
            }
            Value::Set(s) => {
                let mut acc = 0u64;
                for e in s.iter() {
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    e.hash(&mut h);
                    acc ^= h.finish();
                }
                acc.hash(state);
            }
        }
    }
}

// ===== World =====
#[derive(Debug)]
struct World {
    vars: HashMap<String, Value>,
    generation: HashMap<String, usize>,
}

impl World {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            generation: HashMap::new(),
        }
    }
    fn get_gen(&self, var: &str) -> usize {
        *self.generation.get(var).unwrap_or(&0)
    }
    fn inc_gen(&mut self, var: &str) {
        *self.generation.entry(var.to_string()).or_insert(0) += 1;
    }
}

// ===== Branch =====
#[derive(Clone)]
struct Branch {
    variable: String,
    delta: Option<Value>,
    generation: usize,
    nested: Vec<Branch>,
}

impl Branch {
    fn new(variable: &str, delta: Option<Value>, generation: usize) -> Self {
        Self {
            variable: variable.to_string(),
            delta,
            generation,
            nested: vec![],
        }
    }
    fn merge(self, world: &mut World) {
        if world.get_gen(&self.variable) != self.generation {
            return;
        }
        if let Some(val) = self.delta {
            world.vars.insert(self.variable.clone(), val);
        }
        world.inc_gen(&self.variable);
        for nested in self.nested {
            nested.merge(world);
        }
    }
}

// ===== AST =====
#[derive(Debug)]
enum PrintTarget {
    Variable(String),
    Value(Value),
}

#[derive(Debug)]
enum ASTNode {
    Let {
        name: String,
        value: Value,
    },
    Branch {
        variable: String,
        body: Vec<ASTNode>,
    },
    Merge {
        variable: String,
    },
    Print {
        target: PrintTarget,
    },
    Input {
        prompt: Option<String>,
        variable: String,
    },
    ListPush {
        variable: String,
        value: Value,
    },
    SetInsert {
        variable: String,
        value: Value,
    },
}

// ===== Lexer =====
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Let,
    Branch,
    Merge,
    Print,
    Input,
    Identifier(String),
    Number(i32),
    Float(f64),
    Bool(bool),
    Str(String),
    Equals,
    LBrace,
    RBrace,
    Semicolon,
    LBracket,
    RBracket,
    Comma,
}

fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut iter = input.chars().peekable();
    while let Some(&c) = iter.peek() {
        match c {
            c if c.is_whitespace() => {
                iter.next();
            }
            '=' => {
                tokens.push(Token::Equals);
                iter.next();
            }
            '{' => {
                tokens.push(Token::LBrace);
                iter.next();
            }
            '}' => {
                tokens.push(Token::RBrace);
                iter.next();
            }
            '[' => {
                tokens.push(Token::LBracket);
                iter.next();
            }
            ']' => {
                tokens.push(Token::RBracket);
                iter.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                iter.next();
            }
            ';' => {
                tokens.push(Token::Semicolon);
                iter.next();
            }
            '"' => {
                iter.next();
                let mut s = String::new();
                while let Some(&ch) = iter.peek() {
                    if ch == '"' {
                        iter.next();
                        break;
                    }
                    s.push(ch);
                    iter.next();
                }
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let mut num = 0;
                while let Some(&d) = iter.peek() {
                    if d.is_ascii_digit() {
                        num = num * 10 + (d as i32 - '0' as i32);
                        iter.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Number(num));
            }
            c if c.is_ascii_alphabetic() => {
                let mut ident = String::new();
                while let Some(&d) = iter.peek() {
                    if d.is_ascii_alphanumeric() || d == '_' {
                        ident.push(d);
                        iter.next();
                    } else {
                        break;
                    }
                }
                let token = match ident.as_str() {
                    "let" => Token::Let,
                    "branch" => Token::Branch,
                    "merge" => Token::Merge,
                    "print" => Token::Print,
                    "input" => Token::Input,
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    _ => Token::Identifier(ident),
                };
                tokens.push(token);
            }
            _ => {
                iter.next();
            }
        }
    }
    tokens
}

// ===== Parser =====
fn parse_let(tokens: &mut std::slice::Iter<Token>) -> ASTNode {
    if let Some(Token::Identifier(name)) = tokens.next() {
        if let Some(Token::Equals) = tokens.next() {
            let value = match tokens.next() {
                Some(Token::Number(n)) => Value::Int(*n),
                Some(Token::Float(f)) => Value::Float(Float(*f)),
                Some(Token::Bool(b)) => Value::Bool(*b),
                Some(Token::Str(s)) => Value::Str(Arc::new(s.clone())),
                Some(Token::LBracket) => {
                    match tokens.next() {
                        Some(Token::RBracket) => Value::List(Arc::new(Vec::new())), // empty list
                        _ => Value::Set(Arc::new(HashSet::new())), // treat [] as empty set if needed
                    }
                }
                _ => panic!("Invalid let value"),
            };
            let _ = tokens.next(); // optional ;
            return ASTNode::Let {
                name: name.clone(),
                value,
            };
        }
    }
    panic!("Invalid let syntax");
}

fn parse_branch(tokens: &mut std::slice::Iter<Token>) -> ASTNode {
    let variable = match tokens.next() {
        Some(Token::Identifier(name)) => name.clone(),
        _ => panic!("Expected identifier"),
    };
    match tokens.next() {
        Some(Token::LBrace) => {}
        _ => panic!("Expected {{"),
    }
    let mut body = Vec::new();
    while let Some(token) = tokens.next() {
        match token {
            Token::RBrace => break,
            Token::Let => body.push(parse_let(tokens)),
            Token::Branch => body.push(parse_branch(tokens)),
            Token::Merge => {
                if let Some(Token::Identifier(name)) = tokens.next() {
                    let _ = tokens.next();
                    body.push(ASTNode::Merge {
                        variable: name.clone(),
                    });
                }
            }
            Token::Print => match tokens.next() {
                Some(Token::Identifier(name)) => body.push(ASTNode::Print {
                    target: PrintTarget::Variable(name.clone()),
                }),
                Some(Token::Number(n)) => body.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Int(*n)),
                }),
                Some(Token::Float(f)) => body.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Float(Float(*f))),
                }),
                Some(Token::Str(s)) => body.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Str(Arc::new(s.clone()))),
                }),
                _ => panic!("Invalid print target"),
            },
            Token::Input => {
                if let Some(Token::Str(prompt)) = tokens.next() {
                    if let Some(Token::Identifier(var)) = tokens.next() {
                        body.push(ASTNode::Input {
                            prompt: Some(prompt.clone()),
                            variable: var.clone(),
                        });
                    }
                }
            }
            Token::Identifier(ident) if ident == "listpush" => {
                if let Some(Token::Identifier(var)) = tokens.next() {
                    if let Some(Token::Number(n)) = tokens.next() {
                        body.push(ASTNode::ListPush {
                            variable: var.clone(),
                            value: Value::Int(*n),
                        });
                    }
                }
            }
            Token::Identifier(ident) if ident == "setinsert" => {
                if let Some(Token::Identifier(var)) = tokens.next() {
                    if let Some(Token::Number(n)) = tokens.next() {
                        body.push(ASTNode::SetInsert {
                            variable: var.clone(),
                            value: Value::Int(*n),
                        });
                    }
                }
            }
            Token::Semicolon => {}
            _ => {}
        }
    }
    ASTNode::Branch { variable, body }
}

fn parse(tokens: &[Token]) -> Vec<ASTNode> {
    let mut iter = tokens.iter();
    let mut ast = Vec::new();
    while let Some(token) = iter.next() {
        match token {
            Token::Let => ast.push(parse_let(&mut iter)),
            Token::Branch => ast.push(parse_branch(&mut iter)),
            Token::Merge => {
                if let Some(Token::Identifier(name)) = iter.next() {
                    ast.push(ASTNode::Merge {
                        variable: name.clone(),
                    });
                }
            }
            Token::Print => match iter.next() {
                Some(Token::Identifier(name)) => ast.push(ASTNode::Print {
                    target: PrintTarget::Variable(name.clone()),
                }),
                Some(Token::Number(n)) => ast.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Int(*n)),
                }),
                Some(Token::Float(f)) => ast.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Float(Float(*f))),
                }),
                Some(Token::Str(s)) => ast.push(ASTNode::Print {
                    target: PrintTarget::Value(Value::Str(Arc::new(s.clone()))),
                }),
                _ => panic!("Invalid print target"),
            },
            Token::Input => {
                if let Some(Token::Str(prompt)) = iter.next() {
                    if let Some(Token::Identifier(var)) = iter.next() {
                        ast.push(ASTNode::Input {
                            prompt: Some(prompt.clone()),
                            variable: var.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    ast
}

// ===== AST実行 =====
fn execute_ast(ast: &[ASTNode], world: &mut World, branches: &mut HashMap<String, Branch>) {
    for node in ast {
        match node {
            ASTNode::Let { name, value } => {
                world.vars.insert(name.clone(), value.clone());
            }
            ASTNode::Branch { variable, body } => {
                let generation = world.get_gen(variable);
                let mut b = Branch::new(variable, None, generation);
                execute_ast(body, world, branches);
                b.nested.extend(branches.drain().map(|(_, v)| v));
                branches.insert(variable.clone(), b);
            }
            ASTNode::Merge { variable } => {
                if let Some(b) = branches.remove(variable) {
                    b.merge(world);
                }
            }
            ASTNode::Print { target } => match target {
                PrintTarget::Variable(var) => {
                    if let Some(val) = world.vars.get(var) {
                        println!("{:?}", val);
                    } else {
                        println!("(undefined variable {})", var);
                    }
                }
                PrintTarget::Value(val) => {
                    println!("{:?}", val);
                }
            },
            ASTNode::Input { prompt, variable } => {
                if let Some(msg) = prompt {
                    print!("{}", msg);
                    io::stdout().flush().unwrap();
                }
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                world.vars.insert(
                    variable.clone(),
                    Value::Str(Arc::new(input.trim().to_string())),
                );
            }
            ASTNode::ListPush { variable, value } => {
                if let Some(Value::List(l)) = world.vars.get(variable) {
                    let mut new_list = (**l).clone();
                    new_list.push(value.clone());
                    world
                        .vars
                        .insert(variable.clone(), Value::List(Arc::new(new_list)));
                }
            }
            ASTNode::SetInsert { variable, value } => {
                if let Some(Value::Set(s)) = world.vars.get(variable) {
                    let mut new_set = (**s).clone();
                    new_set.insert(value.clone());
                    world
                        .vars
                        .insert(variable.clone(), Value::Set(Arc::new(new_set)));
                }
            }
        }
    }
}

// ===== main =====
fn main() {
    let code = fs::read_to_string((env::args().collect())[1].as_str()).unwrap();

    let tokens = lex(&code);
    let ast = parse(&tokens);
    let mut world = World::new();
    let mut branches = HashMap::new();

    println!("Before execution: {:?}", world);
    execute_ast(&ast, &mut world, &mut branches);
    println!("After execution: {:?}", world);
}
