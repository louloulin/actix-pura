//! Analizador de flujo de trabajo para DataFlare
//!
//! Proporciona funcionalidades para analizar y validar flujos de trabajo.

use std::collections::{HashMap, HashSet};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::dot::{Dot, Config};

use crate::{
    error::{DataFlareError, Result},
    workflow::Workflow,
};

/// Analizador de flujo de trabajo
pub struct WorkflowParser {
    /// Flujo de trabajo a analizar
    workflow: Workflow,
    
    /// Grafo del flujo de trabajo
    graph: DiGraph<String, ()>,
    
    /// Mapa de nodos
    node_map: HashMap<String, NodeIndex>,
}

impl WorkflowParser {
    /// Crea un nuevo analizador de flujo de trabajo
    pub fn new(workflow: Workflow) -> Self {
        Self {
            workflow,
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }
    
    /// Analiza el flujo de trabajo
    pub fn parse(&mut self) -> Result<()> {
        // Limpiar grafo y mapa de nodos
        self.graph = DiGraph::new();
        self.node_map.clear();
        
        // Agregar nodos para fuentes
        for (id, _) in &self.workflow.sources {
            let node_idx = self.graph.add_node(id.clone());
            self.node_map.insert(id.clone(), node_idx);
        }
        
        // Agregar nodos para transformaciones
        for (id, _) in &self.workflow.transformations {
            let node_idx = self.graph.add_node(id.clone());
            self.node_map.insert(id.clone(), node_idx);
        }
        
        // Agregar nodos para destinos
        for (id, _) in &self.workflow.destinations {
            let node_idx = self.graph.add_node(id.clone());
            self.node_map.insert(id.clone(), node_idx);
        }
        
        // Agregar aristas para transformaciones
        for (id, transform) in &self.workflow.transformations {
            let to_idx = self.node_map.get(id).unwrap();
            
            for input in &transform.inputs {
                if let Some(from_idx) = self.node_map.get(input) {
                    self.graph.add_edge(*from_idx, *to_idx, ());
                } else {
                    return Err(DataFlareError::Validation(format!(
                        "Entrada inválida en transformación {}: {}", id, input
                    )));
                }
            }
        }
        
        // Agregar aristas para destinos
        for (id, dest) in &self.workflow.destinations {
            let to_idx = self.node_map.get(id).unwrap();
            
            for input in &dest.inputs {
                if let Some(from_idx) = self.node_map.get(input) {
                    self.graph.add_edge(*from_idx, *to_idx, ());
                } else {
                    return Err(DataFlareError::Validation(format!(
                        "Entrada inválida en destino {}: {}", id, input
                    )));
                }
            }
        }
        
        Ok(())
    }
    
    /// Verifica si el flujo de trabajo tiene ciclos
    pub fn has_cycles(&self) -> bool {
        match toposort(&self.graph, None) {
            Ok(_) => false,
            Err(_) => true,
        }
    }
    
    /// Obtiene el orden topológico de los componentes
    pub fn get_topological_order(&self) -> Result<Vec<String>> {
        match toposort(&self.graph, None) {
            Ok(indices) => {
                let mut result = Vec::new();
                for idx in indices {
                    if let Some(name) = self.graph.node_weight(idx) {
                        result.push(name.clone());
                    }
                }
                Ok(result)
            },
            Err(_) => Err(DataFlareError::Validation("El flujo de trabajo contiene ciclos".to_string())),
        }
    }
    
    /// Verifica si todos los componentes son alcanzables
    pub fn all_components_reachable(&self) -> bool {
        let mut visited = HashSet::new();
        
        // Encontrar nodos de destino
        let dest_nodes: Vec<_> = self.workflow.destinations.keys()
            .filter_map(|id| self.node_map.get(id).cloned())
            .collect();
        
        // Realizar DFS desde cada destino
        for &start in &dest_nodes {
            self.dfs(start, &mut visited);
        }
        
        // Verificar que todos los nodos fueron visitados
        self.node_map.len() == visited.len()
    }
    
    /// Busca componentes no utilizados
    pub fn find_unused_components(&self) -> Vec<String> {
        let mut visited = HashSet::new();
        
        // Encontrar nodos de destino
        let dest_nodes: Vec<_> = self.workflow.destinations.keys()
            .filter_map(|id| self.node_map.get(id).cloned())
            .collect();
        
        // Realizar DFS desde cada destino
        for &start in &dest_nodes {
            self.dfs(start, &mut visited);
        }
        
        // Encontrar nodos no visitados
        let mut unused = Vec::new();
        for (id, idx) in &self.node_map {
            if !visited.contains(idx) {
                unused.push(id.clone());
            }
        }
        
        unused
    }
    
    /// Genera una representación DOT del grafo
    pub fn to_dot(&self) -> String {
        format!("{:?}", Dot::with_config(&self.graph, &[Config::EdgeNoLabel]))
    }
    
    /// Realiza una búsqueda en profundidad
    fn dfs(&self, start: NodeIndex, visited: &mut HashSet<NodeIndex>) {
        if visited.contains(&start) {
            return;
        }
        
        visited.insert(start);
        
        // Visitar todos los predecesores
        for neighbor in self.graph.neighbors_directed(start, petgraph::Incoming) {
            self.dfs(neighbor, visited);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::WorkflowBuilder;
    
    #[test]
    fn test_workflow_parser() {
        // Crear flujo de trabajo
        let workflow = WorkflowBuilder::new("test-workflow", "Test Workflow")
            .source("source1", "memory", serde_json::json!({}))
            .source("source2", "memory", serde_json::json!({}))
            .transformation("transform1", "mapping", vec!["source1"], serde_json::json!({}))
            .transformation("transform2", "mapping", vec!["source2"], serde_json::json!({}))
            .transformation("join", "join", vec!["transform1", "transform2"], serde_json::json!({}))
            .destination("dest", "memory", vec!["join"], serde_json::json!({}))
            .build()
            .unwrap();
        
        // Crear analizador
        let mut parser = WorkflowParser::new(workflow);
        
        // Analizar flujo de trabajo
        parser.parse().unwrap();
        
        // Verificar que no hay ciclos
        assert!(!parser.has_cycles());
        
        // Verificar orden topológico
        let order = parser.get_topological_order().unwrap();
        assert!(order.contains(&"source1".to_string()));
        assert!(order.contains(&"source2".to_string()));
        assert!(order.contains(&"transform1".to_string()));
        assert!(order.contains(&"transform2".to_string()));
        assert!(order.contains(&"join".to_string()));
        assert!(order.contains(&"dest".to_string()));
        
        // Verificar que todos los componentes son alcanzables
        assert!(parser.all_components_reachable());
        
        // Verificar que no hay componentes no utilizados
        assert!(parser.find_unused_components().is_empty());
    }
    
    #[test]
    fn test_workflow_with_unused_components() {
        // Crear flujo de trabajo con componente no utilizado
        let workflow = WorkflowBuilder::new("test-workflow", "Test Workflow")
            .source("source1", "memory", serde_json::json!({}))
            .source("source2", "memory", serde_json::json!({})) // No utilizado
            .transformation("transform", "mapping", vec!["source1"], serde_json::json!({}))
            .destination("dest", "memory", vec!["transform"], serde_json::json!({}))
            .build()
            .unwrap();
        
        // Crear analizador
        let mut parser = WorkflowParser::new(workflow);
        
        // Analizar flujo de trabajo
        parser.parse().unwrap();
        
        // Verificar que no hay ciclos
        assert!(!parser.has_cycles());
        
        // Verificar que no todos los componentes son alcanzables
        assert!(!parser.all_components_reachable());
        
        // Verificar componentes no utilizados
        let unused = parser.find_unused_components();
        assert_eq!(unused.len(), 1);
        assert!(unused.contains(&"source2".to_string()));
    }
    
    #[test]
    fn test_workflow_with_cycles() {
        // Crear flujo de trabajo con ciclo
        let mut workflow = WorkflowBuilder::new("test-workflow", "Test Workflow")
            .source("source", "memory", serde_json::json!({}))
            .transformation("transform1", "mapping", vec!["source"], serde_json::json!({}))
            .transformation("transform2", "mapping", vec!["transform1"], serde_json::json!({}))
            .destination("dest", "memory", vec!["transform2"], serde_json::json!({}))
            .build()
            .unwrap();
        
        // Agregar ciclo (transform2 -> transform1)
        if let Some(transform) = workflow.transformations.get_mut("transform1") {
            transform.inputs.push("transform2".to_string());
        }
        
        // Crear analizador
        let mut parser = WorkflowParser::new(workflow);
        
        // Analizar flujo de trabajo
        parser.parse().unwrap();
        
        // Verificar que hay ciclos
        assert!(parser.has_cycles());
        
        // Verificar que el orden topológico falla
        assert!(parser.get_topological_order().is_err());
    }
}
