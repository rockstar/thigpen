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
    relations: RefCell<Vec<(String, String)>>,
}

impl Visitor for DotVisualization {
    fn visit(&self, module: &Module) {
        for child in &module.children {
            self.relations
                .borrow_mut()
                .push((module.usepath.clone(), child.usepath.clone()));
            self.visit(child);
        }
    }
}

impl Visualization for DotVisualization {
    fn new(project: Project) -> Self {
        Self {
            project,
            relations: RefCell::new(vec![]),
        }
    }

    fn visualize(&self, writer: &mut dyn std::io::Write) {
        self.visit(&self.project.module);

        writeln!(writer, "graph relations {{").unwrap();
        for relation in self.relations.borrow().iter() {
            writeln!(writer, "\"{}\" -- \"{}\"", relation.0, relation.1).unwrap();
        }
        writeln!(writer, "}}").unwrap();
    }
}
