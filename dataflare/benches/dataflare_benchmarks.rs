//! Benchmarks para DataFlare

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dataflare::{
    message::DataRecord,
    processor::{MappingProcessor, Processor, ProcessorState},
};
use serde_json::json;

/// Benchmark para el procesador de mapeo
fn bench_mapping_processor(c: &mut Criterion) {
    // Crear procesador
    let mut processor = MappingProcessor::new();
    
    // Configurar procesador
    processor.configure(&json!({
        "mappings": [
            {
                "source": "name",
                "destination": "user.name",
                "transform": "uppercase"
            },
            {
                "source": "email",
                "destination": "user.email",
                "transform": "lowercase"
            },
            {
                "source": "age",
                "destination": "user.age"
            }
        ]
    })).unwrap();
    
    // Crear registro de prueba
    let record = DataRecord::new(json!({
        "name": "John Doe",
        "email": "John.Doe@Example.com",
        "age": 30
    }));
    
    // Benchmark
    c.bench_function("mapping_processor", |b| {
        b.iter(|| {
            let fut = processor.process_record(black_box(&record), None);
            futures::executor::block_on(fut).unwrap();
        });
    });
}

criterion_group!(benches, bench_mapping_processor);
criterion_main!(benches);
