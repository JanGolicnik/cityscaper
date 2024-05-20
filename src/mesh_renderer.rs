use std::collections::HashMap;

use jandering_engine::{
    core::{
        object::{Instance, Renderable},
        renderer::{RenderPass, Renderer},
    },
    utils::load_text,
};

use crate::color_obj::ColorObject;

macro_rules! def_enum {
    (
        $(#[$attr:meta])*
        $vis:vis $name:ident => $ty:ty {
            $($variant:ident => $val:expr),+
            $(,)?
        }
    ) => {
        #[derive(Copy, Clone)]
        $(#[$attr])*
        $vis struct $name($ty);

        #[allow(non_upper_case_globals, dead_code)]
        impl $name {
            $(
                pub const $variant: Self = Self($val);
            )+

            const VARIANTS: &'static [Self] = &[$(Self::$variant),+];

            pub fn iter() -> std::slice::Iter<'static, Self> {
                Self::VARIANTS.iter()
            }

            pub const fn get(self) -> $ty {
                self.0
            }
        }
    };
}

def_enum!(
    #[derive(Debug, Eq, PartialEq, Hash)]
    pub Mesh => (usize, &'static str) {
        Empty => (0, "empty"),
        House1 => (1, "house1"),
        Intersection => (2, "intersection"),
        Road => (3, "road"),
    }
);

pub struct MeshRenderer {
    meshes: Vec<ColorObject>,
    queued: HashMap<Mesh, Vec<Instance>>,
}

impl MeshRenderer {
    pub async fn new(renderer: &mut dyn Renderer) -> Self {
        let mut meshes = Vec::new();

        for mesh in Mesh::iter() {
            let file_name = format!("{}.obj", mesh.get().1);
            let file = load_text(jandering_engine::utils::FilePath::FileName(&file_name))
                .await
                .unwrap();
            meshes.push(ColorObject::from_obj(&file, renderer, Vec::new()));
        }

        let queued = HashMap::new();

        Self { meshes, queued }
    }

    pub fn update(&mut self, renderer: &mut dyn Renderer) {
        self.meshes.iter_mut().for_each(|e| e.instances.clear());
        for (mesh, instances) in self.queued.iter_mut() {
            let mesh = &mut self.meshes[mesh.get().0];
            mesh.instances = std::mem::take(instances);
            mesh.update(renderer);
        }
        self.queued.clear();
    }

    pub fn render_mesh(&mut self, mesh: Mesh, mut instances: Vec<Instance>) {
        self.queued
            .entry(mesh)
            .and_modify(|e| e.append(&mut instances))
            .or_insert(instances);
    }

    pub fn bind_meshes<'a>(
        &'a mut self,
        render_pass: Box<dyn RenderPass<'a> + 'a>,
    ) -> Box<dyn RenderPass + 'a> {
        let meshes: Vec<&dyn Renderable> =
            self.meshes.iter().map(|e| e as &dyn Renderable).collect();
        render_pass.render(&meshes)
    }
}
