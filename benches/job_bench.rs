// Benchmarks for async job operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;
use npio::backend::local::LocalBackend;
use npio::{get_file_for_uri, register_backend, CopyFlags};
use npio::job;

fn setup_test_environment() -> (Runtime, PathBuf) {
    let rt = Runtime::new().unwrap();
    let test_dir = std::env::temp_dir().join("npio_bench");
    
    // Cleanup if exists
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).ok();
    }
    std::fs::create_dir_all(&test_dir).unwrap();
    
    (rt, test_dir)
}

fn create_test_file(rt: &Runtime, path: &PathBuf, size: usize) {
    rt.block_on(async {
        let content = vec![b'a'; size];
        tokio::fs::write(path, &content).await.unwrap();
    });
}

fn bench_copy_job(c: &mut Criterion) {
    let (rt, test_dir) = setup_test_environment();
    
    rt.block_on(async {
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
    });
    
    let mut group = c.benchmark_group("copy_job");
    
    for size in [1024, 1024 * 10, 1024 * 100, 1024 * 1024].iter() {
        let src_path = test_dir.join(format!("src_{}.txt", size));
        let dest_path = test_dir.join(format!("dest_{}.txt", size));
        
        create_test_file(&rt, &src_path, *size);
        
        let src_uri = format!("file://{}", src_path.to_string_lossy());
        let dest_uri = format!("file://{}", dest_path.to_string_lossy());
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let src_file = get_file_for_uri(&src_uri).unwrap();
                    let dest_file = get_file_for_uri(&dest_uri).unwrap();
                    
                    // Clean destination
                    if dest_path.exists() {
                        tokio::fs::remove_file(&dest_path).await.ok();
                    }
                    
                    job::copy(
                        &*src_file,
                        &*dest_file,
                        CopyFlags::NONE,
                        None,
                        None,
                    )
                    .await
                    .unwrap();
                });
            },
        );
    }
    
    group.finish();
    
    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

fn bench_move_job(c: &mut Criterion) {
    let (rt, test_dir) = setup_test_environment();
    
    rt.block_on(async {
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
    });
    
    let mut group = c.benchmark_group("move_job");
    
    for size in [1024, 1024 * 10, 1024 * 100].iter() {
        let src_path = test_dir.join(format!("src_{}.txt", size));
        let dest_path = test_dir.join(format!("dest_{}.txt", size));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    // Create source file
                    create_test_file(&rt, &src_path, *size);
                    
                    let src_uri = format!("file://{}", src_path.to_string_lossy());
                    let dest_uri = format!("file://{}", dest_path.to_string_lossy());
                    
                    let src_file = get_file_for_uri(&src_uri).unwrap();
                    let dest_file = get_file_for_uri(&dest_uri).unwrap();
                    
                    job::move_(
                        &*src_file,
                        &*dest_file,
                        CopyFlags::NONE,
                        None,
                        None,
                    )
                    .await
                    .unwrap();
                });
            },
        );
    }
    
    group.finish();
    
    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

fn bench_delete_job(c: &mut Criterion) {
    let (rt, test_dir) = setup_test_environment();
    
    rt.block_on(async {
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
    });
    
    let mut group = c.benchmark_group("delete_job");
    
    for size in [1024, 1024 * 10, 1024 * 100].iter() {
        let file_path = test_dir.join(format!("file_{}.txt", size));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    // Create file
                    create_test_file(&rt, &file_path, *size);
                    
                    let file_uri = format!("file://{}", file_path.to_string_lossy());
                    let file = get_file_for_uri(&file_uri).unwrap();
                    
                    job::delete(&*file, None).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
    
    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

fn bench_file_read(c: &mut Criterion) {
    let (rt, test_dir) = setup_test_environment();
    
    rt.block_on(async {
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
    });
    
    let mut group = c.benchmark_group("file_read");
    
    for size in [1024, 1024 * 10, 1024 * 100, 1024 * 1024].iter() {
        let file_path = test_dir.join(format!("read_{}.txt", size));
        create_test_file(&rt, &file_path, *size);
        
        let file_uri = format!("file://{}", file_path.to_string_lossy());
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let file = get_file_for_uri(&file_uri).unwrap();
                    let mut input = file.read(None).await.unwrap();
                    let mut buffer = Vec::new();
                    tokio::io::AsyncReadExt::read_to_end(&mut input, &mut buffer)
                        .await
                        .unwrap();
                    black_box(buffer);
                });
            },
        );
    }
    
    group.finish();
    
    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

fn bench_file_write(c: &mut Criterion) {
    let (rt, test_dir) = setup_test_environment();
    
    rt.block_on(async {
        let backend = Arc::new(LocalBackend::new());
        register_backend(backend);
    });
    
    let mut group = c.benchmark_group("file_write");
    
    for size in [1024, 1024 * 10, 1024 * 100, 1024 * 1024].iter() {
        let file_path = test_dir.join(format!("write_{}.txt", size));
        let content = vec![b'b'; *size];
        
        let file_uri = format!("file://{}", file_path.to_string_lossy());
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let file = get_file_for_uri(&file_uri).unwrap();
                    let mut output = file.create_file(None).await.unwrap();
                    output.write_all(&content).await.unwrap();
                    output.close(None).unwrap();
                    
                    // Clean up for next iteration
                    tokio::fs::remove_file(&file_path).await.ok();
                });
            },
        );
    }
    
    group.finish();
    
    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

criterion_group!(
    benches,
    bench_copy_job,
    bench_move_job,
    bench_delete_job,
    bench_file_read,
    bench_file_write
);
criterion_main!(benches);

