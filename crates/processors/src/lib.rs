#![forbid(unsafe_code)]

pub mod ast;
pub mod binary;
pub mod containers;
pub mod decompiler;
pub mod html;
pub mod images;
pub mod markdown;
pub mod network;
pub mod ocr;
pub mod office;
pub mod pdf;
pub mod processing_fabric;
pub mod repositories;
pub mod video;

use tordex_core::processor::ProcessorRegistry;

pub use ast::AstProcessor;
pub use binary::BinaryProcessor;
pub use containers::ContainerProcessor;
pub use decompiler::DecompilerProcessor;
pub use html::HtmlProcessor;
pub use images::ImageProcessor;
pub use markdown::MarkdownProcessor;
pub use network::NetworkProcessor;
pub use ocr::OcrProcessor;
pub use office::OfficeProcessor;
pub use pdf::PdfProcessor;
pub use processing_fabric::ProcessingFabric;
pub use repositories::RepositoryProcessor;
pub use video::VideoProcessor;

/// Register all built-in processors into a `ProcessorRegistry`.
///
/// Call this during application startup to make all 13 processing
/// capabilities available through the fabric.
pub fn register_all(registry: &dyn ProcessorRegistry) {
    let processors: Vec<Box<dyn tordex_core::processor::Processor>> = vec![
        Box::new(HtmlProcessor::new()),
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
    ];
    for proc in processors {
        let _ = registry.register(proc);
    }
}
