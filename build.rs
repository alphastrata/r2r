use common;
use msg_gen;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    common::print_cargo_watches();

    let msg_list = if let Some(cmake_includes) = env::var("CMAKE_INCLUDE_DIRS").ok() {
        let packages = cmake_includes
            .split(":")
            .flat_map(|i| Path::new(i).parent())
            .collect::<Vec<_>>();
        let deps = env::var("CMAKE_IDL_PACKAGES").unwrap_or(String::default());
        let deps = deps.split(":").collect::<Vec<_>>();
        let msgs = common::get_ros_msgs(&packages);
        common::parse_msgs(&msgs)
            .into_iter()
            .filter(|msg| deps.contains(&msg.module.as_str()))
            .collect::<Vec<_>>()
    } else {
        let ament_prefix_var = env::var("AMENT_PREFIX_PATH").expect("Source your ROS!");
        let paths = ament_prefix_var
            .split(":")
            .map(|i| Path::new(i))
            .collect::<Vec<_>>();
        let msgs = common::get_ros_msgs(&paths);
        common::parse_msgs(&msgs)
    };
    let msgs = common::as_map(&msg_list);

    let mut modules = String::new();

    for (module, prefixes) in &msgs {
        modules.push_str(&format!(
            r#"pub mod {module}{{include!(concat!(env!("OUT_DIR"), "/{module}.rs"));}}{lf}"#,
            module = module,
            lf = "\n"
        ));

        let mut codegen = String::new();
        for (prefix, msgs) in prefixes {
            codegen.push_str(&format!("  pub mod {} {{\n", prefix));

            if prefix == &"action" {
                for msg in msgs {
                    codegen.push_str("#[allow(non_snake_case)]\n");
                    codegen.push_str(&format!("    pub mod {} {{\n", msg));
                    codegen.push_str("    use super::super::super::*;\n");

                    codegen.push_str(&msg_gen::generate_rust_action(module, prefix, msg));

                    for s in &["Goal", "Result", "Feedback"] {
                        let msgname = format!("{}_{}", msg, s);
                        codegen.push_str(&msg_gen::generate_rust_msg(module, prefix, &msgname));
                        println!("cargo:rustc-cfg=r2r__{}__{}__{}", module, prefix, msg);
                    }

                    // "internal" services that implements the action type
                    for srv in &["SendGoal", "GetResult"] {
                        codegen.push_str("#[allow(non_snake_case)]\n");
                        codegen.push_str(&format!("    pub mod {} {{\n", srv));
                        codegen.push_str("    use super::super::super::super::*;\n");

                        let srvname = format!("{}_{}", msg, srv);
                        codegen.push_str(&msg_gen::generate_rust_service(module, prefix, &srvname));

                        for s in &["Request", "Response"] {
                            let msgname = format!("{}_{}_{}", msg, srv, s);
                            codegen.push_str(&msg_gen::generate_rust_msg(module, prefix, &msgname));
                        }
                        codegen.push_str("    }\n");
                    }

                    // also "internal" feedback message type that wraps the feedback type with a uuid
                    let feedback_msgname = format!("{}_FeedbackMessage", msg);
                    codegen.push_str(&msg_gen::generate_rust_msg(
                        module,
                        prefix,
                        &feedback_msgname,
                    ));

                    codegen.push_str("    }\n");
                }
            } else if prefix == &"srv" {
                for msg in msgs {
                    codegen.push_str("#[allow(non_snake_case)]\n");
                    codegen.push_str(&format!("    pub mod {} {{\n", msg));
                    codegen.push_str("    use super::super::super::*;\n");

                    codegen.push_str(&msg_gen::generate_rust_service(module, prefix, msg));

                    for s in &["Request", "Response"] {
                        let msgname = format!("{}_{}", msg, s);
                        codegen.push_str(&msg_gen::generate_rust_msg(module, prefix, &msgname));
                        println!("cargo:rustc-cfg=r2r__{}__{}__{}", module, prefix, msg);
                    }
                    codegen.push_str("    }\n");
                }
            } else if prefix == &"msg" {
                codegen.push_str("    use super::super::*;\n");
                for msg in msgs {
                    codegen.push_str(&msg_gen::generate_rust_msg(module, prefix, msg));
                    println!("cargo:rustc-cfg=r2r__{}__{}__{}", module, prefix, msg);
                }
            } else {
                panic!("unknown prefix type: {}", prefix);
            }

            codegen.push_str("  }\n");
        }
        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        let mod_fn = out_path.join(&format!("{}.rs", module));
        let mut f = File::create(mod_fn).unwrap();
        write!(f, "{}", codegen).unwrap();
    }

    let untyped_helper = msg_gen::generate_untyped_helper(&msg_list);

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let msgs_fn = out_path.join("_r2r_generated_msgs.rs");
    let untyped_fn = out_path.join("_r2r_generated_untyped_helper.rs");

    let mut f = File::create(msgs_fn).unwrap();
    write!(f, "{}", modules).unwrap();
    let mut f = File::create(untyped_fn).unwrap();
    write!(f, "{}", untyped_helper).unwrap();
}
