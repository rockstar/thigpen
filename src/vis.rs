use std::cell::RefCell;

use crate::project::{Module, Project};

trait Visitor {
    fn visit(&self, module: &Module);
}

pub trait Visualization {
    fn new(project: Project) -> Self;

    fn visualize(&self, writer: &mut dyn std::io::Write);
}

pub struct DotVisualization {
    project: Project,
    children: RefCell<Vec<(String, String)>>,
    dependencies: RefCell<Vec<(String, String)>>,
}

impl Visitor for DotVisualization {
    fn visit(&self, module: &Module) {
        for child in &module.children {
            self.children
                .borrow_mut()
                .push((module.usepath.clone(), child.usepath.clone()));
            self.visit(child);
        }

        // Add dependency relations (this module -> modules it depends on)
        for dependency in &module.dependencies {
            self.dependencies
                .borrow_mut()
                .push((module.usepath.clone(), dependency.target_module.clone()));
        }
    }
}

impl Visualization for DotVisualization {
    fn new(project: Project) -> Self {
        Self {
            project,
            children: RefCell::new(vec![]),
            dependencies: RefCell::new(vec![]),
        }
    }

    fn visualize(&self, writer: &mut dyn std::io::Write) {
        self.visit(&self.project.module);

        writeln!(writer, "graph relations {{").unwrap();
        writeln!(writer, "  // Node styling").unwrap();
        writeln!(writer, "  node [shape=box, style=rounded];").unwrap();
        writeln!(writer).unwrap();

        // Draw hierarchy relations (solid black lines)
        writeln!(writer, "  // Module hierarchy").unwrap();
        for (from, to) in self.children.borrow().iter() {
            writeln!(
                writer,
                "  \"{}\" -- \"{}\" [color=black, style=solid];",
                from, to
            )
            .unwrap();
        }

        writeln!(writer).unwrap();

        // Draw dependency relations (dashed red lines)
        writeln!(writer, "  // Dependencies").unwrap();
        for (from, to) in self.dependencies.borrow().iter() {
            writeln!(
                writer,
                "  \"{}\" -- \"{}\" [color=red, style=dashed];",
                from, to
            )
            .unwrap();
        }

        writeln!(writer, "}}").unwrap();
    }
}
