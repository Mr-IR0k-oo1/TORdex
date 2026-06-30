#![forbid(unsafe_code)]

pub mod algorithm_engine;
pub mod ast;
pub mod binary;
pub mod category_theory;
pub mod containers;
pub mod decompiler;
pub mod graph_theory;
pub mod html;
pub mod images;
pub mod information_theory;
pub mod knowledge_processor;
pub mod markdown;
pub mod network;
pub mod ocr;
pub mod office;
pub mod optimization;
pub mod pdf;
pub mod probability;
pub mod processing_fabric;
pub mod repo_intel_processor;
pub mod repositories;
pub mod set_theory;
pub mod temporal_algebra;
pub mod temporal_graph_processor;
pub mod vector_spaces;
pub mod video;

use tordex_core::processor::ProcessorRegistry;

pub use algorithm_engine::AlgorithmEngine;
pub use ast::AstProcessor;
pub use binary::BinaryProcessor;
pub use category_theory::CategoryTheoryProcessor;
pub use containers::ContainerProcessor;
pub use decompiler::DecompilerProcessor;
pub use graph_theory::GraphTheoryProcessor;
pub use html::HtmlProcessor;
pub use images::ImageProcessor;
pub use information_theory::InformationTheoryProcessor;
pub use knowledge_processor::KnowledgeProcessor;
pub use markdown::MarkdownProcessor;
pub use network::NetworkProcessor;
pub use ocr::OcrProcessor;
pub use office::OfficeProcessor;
pub use optimization::OptimizationProcessor;
pub use pdf::PdfProcessor;
pub use probability::ProbabilityProcessor;
pub use processing_fabric::ProcessingFabric;
pub use repo_intel_processor::RepoIntelProcessor;
pub use repositories::RepositoryProcessor;
pub use set_theory::SetTheoryProcessor;
pub use temporal_algebra::TemporalAlgebraProcessor;
pub use temporal_graph_processor::TemporalGraphProcessor;
pub use vector_spaces::VectorSpaceProcessor;
pub use video::VideoProcessor;

/// Register all built-in processors into a `ProcessorRegistry`.
///
/// Call this during application startup to make all processing
/// capabilities available through the fabric.
pub fn register_all(registry: &dyn ProcessorRegistry) {
    let processors: Vec<Box<dyn tordex_core::processor::Processor>> = vec![
        Box::new(HtmlProcessor::new()),
        Box::new(KnowledgeProcessor::new()),
        Box::new(MarkdownProcessor::new()),
        Box::new(PdfProcessor::new()),
        Box::new(OfficeProcessor::new()),
        Box::new(ImageProcessor::new()),
        Box::new(VideoProcessor::new()),
        Box::new(OcrProcessor::new()),
        Box::new(AstProcessor::new()),
        Box::new(RepositoryProcessor::new()),
        Box::new(ContainerProcessor::new()),
        Box::new(NetworkProcessor::new()),
        Box::new(BinaryProcessor::new()),
        Box::new(DecompilerProcessor::new()),
        Box::new(AlgorithmEngine::new()),
        Box::new(GraphTheoryProcessor::new()),
        Box::new(ProbabilityProcessor::new()),
        Box::new(SetTheoryProcessor::new()),
        Box::new(CategoryTheoryProcessor::new()),
        Box::new(OptimizationProcessor::new()),
        Box::new(InformationTheoryProcessor::new()),
        Box::new(VectorSpaceProcessor::new()),
        Box::new(TemporalAlgebraProcessor::new()),
        Box::new(TemporalGraphProcessor::new()),
        Box::new(RepoIntelProcessor::new()),
    ];
    for proc in processors {
        let _ = registry.register(proc);
    }
}
