use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use tx2_pack::{
    format::{PackedSnapshot, ComponentArchetype, ComponentData, StructOfArraysData, FieldType, FieldArray},
    storage::{SnapshotWriter, SnapshotReader},
    compression::CompressionCodec,
};

#[cfg(feature = "encryption")]
use tx2_pack::encryption::EncryptionKey;

fn create_test_snapshot(entity_count: usize, fields_per_entity: usize) -> PackedSnapshot {
    let mut snapshot = PackedSnapshot::new();

    let mut archetype = ComponentArchetype {
        component_id: "Position".to_string(),
        entity_ids: Vec::with_capacity(entity_count),
        data: ComponentData::StructOfArrays(StructOfArraysData {
            field_names: vec![
                "x".to_string(),
                "y".to_string(),
                "z".to_string(),
            ],
            field_types: vec![
                FieldType::F32,
                FieldType::F32,
                FieldType::F32,
            ],
            field_data: vec![
                FieldArray::F32(Vec::with_capacity(entity_count)),
                FieldArray::F32(Vec::with_capacity(entity_count)),
                FieldArray::F32(Vec::with_capacity(entity_count)),
            ],
        }),
    };

    for i in 0..entity_count {
        let entity_id = i as u32;
        archetype.entity_ids.push(entity_id);

        if let ComponentData::StructOfArrays(ref mut soa) = archetype.data {
            if let FieldArray::F32(ref mut x_values) = soa.field_data[0] {
                x_values.push(i as f32 * 1.5);
            }
            if let FieldArray::F32(ref mut y_values) = soa.field_data[1] {
                y_values.push(i as f32 * 2.5);
            }
            if let FieldArray::F32(ref mut z_values) = soa.field_data[2] {
                z_values.push(i as f32 * 3.5);
            }
        }
    }

    snapshot.archetypes.push(archetype);
    snapshot.header.entity_count = entity_count as u64;
    snapshot.header.component_count = 1;
    snapshot.header.archetype_count = 1;

    if fields_per_entity > 3 {
        let mut velocity_archetype = ComponentArchetype {
            component_id: "Velocity".to_string(),
            entity_ids: Vec::with_capacity(entity_count),
            data: ComponentData::StructOfArrays(StructOfArraysData {
                field_names: vec![
                    "vx".to_string(),
                    "vy".to_string(),
                    "vz".to_string(),
                ],
                field_types: vec![
                    FieldType::F32,
                    FieldType::F32,
                    FieldType::F32,
                ],
                field_data: vec![
                    FieldArray::F32(Vec::with_capacity(entity_count)),
                    FieldArray::F32(Vec::with_capacity(entity_count)),
                    FieldArray::F32(Vec::with_capacity(entity_count)),
                ],
            }),
        };

        for i in 0..entity_count {
            let entity_id = i as u32;
            velocity_archetype.entity_ids.push(entity_id);

            if let ComponentData::StructOfArrays(ref mut soa) = velocity_archetype.data {
                if let FieldArray::F32(ref mut vx_values) = soa.field_data[0] {
                    vx_values.push(i as f32 * 0.1);
                }
                if let FieldArray::F32(ref mut vy_values) = soa.field_data[1] {
                    vy_values.push(i as f32 * 0.2);
                }
                if let FieldArray::F32(ref mut vz_values) = soa.field_data[2] {
                    vz_values.push(i as f32 * 0.3);
                }
            }
        }

        snapshot.archetypes.push(velocity_archetype);
        snapshot.header.component_count = 2;
        snapshot.header.archetype_count = 2;
    }

    snapshot
}

fn bench_write_no_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_no_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::none());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });
    }

    group.finish();
}

fn bench_write_zstd_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_zstd_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::zstd_default());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });
    }

    group.finish();
}

fn bench_write_lz4_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_lz4_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::lz4_default());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });
    }

    group.finish();
}

fn bench_read_no_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_no_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        let writer = SnapshotWriter::new()
            .with_compression(CompressionCodec::none());
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let reader = SnapshotReader::new();

            b.iter(|| {
                let loaded = reader.read_from_bytes(black_box(&bytes)).unwrap();
                black_box(loaded);
            });
        });
    }

    group.finish();
}

fn bench_read_zstd_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_zstd_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        let writer = SnapshotWriter::new()
            .with_compression(CompressionCodec::zstd_default());
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let reader = SnapshotReader::new();

            b.iter(|| {
                let loaded = reader.read_from_bytes(black_box(&bytes)).unwrap();
                black_box(loaded);
            });
        });
    }

    group.finish();
}

fn bench_read_lz4_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_lz4_compression");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        let writer = SnapshotWriter::new()
            .with_compression(CompressionCodec::lz4_default());
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let reader = SnapshotReader::new();

            b.iter(|| {
                let loaded = reader.read_from_bytes(black_box(&bytes)).unwrap();
                black_box(loaded);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "encryption")]
fn bench_write_encrypted(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_encrypted");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        let key = EncryptionKey::generate();
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::zstd_default())
                .with_encryption(key.clone());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });
    }

    group.finish();
}

#[cfg(feature = "encryption")]
fn bench_read_encrypted(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_encrypted");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        let key = EncryptionKey::generate();
        let writer = SnapshotWriter::new()
            .with_compression(CompressionCodec::zstd_default())
            .with_encryption(key.clone());
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let reader = SnapshotReader::new()
                .with_encryption(key.clone());

            b.iter(|| {
                let loaded = reader.read_from_bytes(black_box(&bytes)).unwrap();
                black_box(loaded);
            });
        });
    }

    group.finish();
}

fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratio");
    group.sample_size(20);

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 6);

        group.bench_with_input(BenchmarkId::new("none", size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::none());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });

        group.bench_with_input(BenchmarkId::new("zstd", size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::zstd_default());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });

        group.bench_with_input(BenchmarkId::new("lz4", size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::lz4_default());

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                black_box(bytes);
            });
        });
    }

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    for size in [100, 1000, 10000].iter() {
        let snapshot = create_test_snapshot(*size, 3);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let writer = SnapshotWriter::new()
                .with_compression(CompressionCodec::zstd_default());
            let reader = SnapshotReader::new();

            b.iter(|| {
                let bytes = writer.write_to_bytes(black_box(&snapshot)).unwrap();
                let loaded = reader.read_from_bytes(black_box(&bytes)).unwrap();
                black_box(loaded);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_write_no_compression,
    bench_write_zstd_compression,
    bench_write_lz4_compression,
    bench_read_no_compression,
    bench_read_zstd_compression,
    bench_read_lz4_compression,
    bench_compression_ratio,
    bench_roundtrip,
);

#[cfg(feature = "encryption")]
criterion_group!(
    encryption_benches,
    bench_write_encrypted,
    bench_read_encrypted,
);

#[cfg(feature = "encryption")]
criterion_main!(benches, encryption_benches);

#[cfg(not(feature = "encryption"))]
criterion_main!(benches);
