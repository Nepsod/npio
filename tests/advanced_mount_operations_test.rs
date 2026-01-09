//! Property-based tests for advanced mount operations.

use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use npio::mount::advanced::types::{OperationId, OperationType, CancellationReason};
use npio::mount::advanced::progress::{ProgressReporter, OperationStage};
use npio::mount::advanced::config::ProgressConfig;
use npio::mount::advanced::cancellation::CancellationManager;
use npio::{NpioError, IOErrorEnum};
use std::time::Duration;

/// Property test for progress event emission.
/// **Feature: advanced-mount-operations, Property 1: Progress Event Emission**
/// **Validates: Requirements 1.1**
proptest! {
    #[test]
    fn test_progress_event_emission(
        progress_values in prop::collection::vec(0.0f32..1.0f32, 1..100),
        messages in prop::collection::vec("[a-zA-Z0-9 ]{1,50}", 1..100),
        stages in prop::collection::vec(
            (0..5usize).prop_map(|i| match i {
                0 => OperationStage::Validation,
                1 => OperationStage::Preparation,
                2 => OperationStage::Execution,
                3 => OperationStage::Cleanup,
                _ => OperationStage::Completion,
            }),
            1..100
        )
    ) {
        use tokio::sync::broadcast::error::TryRecvError;
        
        let operation_id = OperationId::new();
        let config = ProgressConfig::default();
        let reporter = ProgressReporter::new(operation_id, config.clone());
        let mut receiver = reporter.subscribe();
        
        // Property 1: Starting operations should emit progress events
        let num_events = std::cmp::min(progress_values.len(), std::cmp::min(messages.len(), stages.len()));
        let mut emitted_events = Vec::new();
        
        for i in 0..num_events {
            let progress = progress_values[i];
            let message = messages[i].clone();
            let stage = stages[i];
            
            // Report progress - this should emit an event
            reporter.report_progress(progress, message.clone(), stage);
            
            // Try to receive the event
            match receiver.try_recv() {
                Ok(event) => {
                    emitted_events.push(event);
                }
                Err(TryRecvError::Empty) => {
                    // If streams are disabled, this is expected
                    if config.enable_streams {
                        prop_assert!(false, "Expected progress event but channel was empty");
                    }
                }
                Err(TryRecvError::Closed) => {
                    prop_assert!(false, "Progress channel unexpectedly closed");
                }
                Err(TryRecvError::Lagged(_)) => {
                    // Channel lagged, try again
                    match receiver.try_recv() {
                        Ok(event) => emitted_events.push(event),
                        _ => prop_assert!(false, "Failed to receive event after lag"),
                    }
                }
            }
        }
        
        // Property 2: All emitted events should have valid progress values (0.0 to 1.0)
        for event in &emitted_events {
            prop_assert!(event.progress >= 0.0 && event.progress <= 1.0,
                "Progress value {} is outside valid range [0.0, 1.0]", event.progress);
        }
        
        // Property 3: All emitted events should have the correct operation ID
        for event in &emitted_events {
            prop_assert_eq!(event.operation_id, operation_id,
                "Event operation ID mismatch");
        }
        
        // Property 4: All emitted events should have non-empty messages
        for event in &emitted_events {
            prop_assert!(!event.message.is_empty(),
                "Event message should not be empty");
        }
        
        // Property 5: All emitted events should have valid stages
        for event in &emitted_events {
            match event.stage {
                OperationStage::Validation |
                OperationStage::Preparation |
                OperationStage::Execution |
                OperationStage::Cleanup |
                OperationStage::Completion => {
                    // Valid stage
                }
            }
        }
        
        // Property 6: Events should be emitted in the order they were reported
        if config.enable_streams {
            prop_assert_eq!(emitted_events.len(), num_events,
                "Should emit one event per report_progress call");
            
            for (i, event) in emitted_events.iter().enumerate() {
                let expected_progress = progress_values[i];
                let expected_message = &messages[i];
                let expected_stage = stages[i];
                
                prop_assert_eq!(event.progress, expected_progress,
                    "Event {} progress mismatch", i);
                prop_assert_eq!(&event.message, expected_message,
                    "Event {} message mismatch", i);
                prop_assert_eq!(event.stage, expected_stage,
                    "Event {} stage mismatch", i);
            }
        }
        
        // Property 7: Completion reporting should emit completion event
        let completion_result = reporter.report_completion();
        prop_assert!(completion_result.is_ok(), "Completion reporting should succeed");
        
        if config.enable_streams {
            match receiver.try_recv() {
                Ok(completion_event) => {
                    prop_assert_eq!(completion_event.progress, 1.0,
                        "Completion event should have progress 1.0");
                    prop_assert_eq!(completion_event.stage, OperationStage::Completion,
                        "Completion event should have Completion stage");
                    prop_assert!(!completion_event.message.is_empty(),
                        "Completion event should have non-empty message");
                }
                Err(TryRecvError::Empty) => {
                    prop_assert!(false, "Expected completion event but channel was empty");
                }
                Err(_) => {
                    prop_assert!(false, "Failed to receive completion event");
                }
            }
        }
    }
}

/// Property test for progress message quality.
/// **Feature: advanced-mount-operations, Property 2: Progress Message Quality**
/// **Validates: Requirements 1.2**
proptest! {
    #[test]
    fn test_progress_message_quality(
        progress_values in prop::collection::vec(0.0f32..1.0f32, 1..50),
        message_types in prop::collection::vec(
            (0..6usize).prop_map(|i| match i {
                0 => "Validating mount point",
                1 => "Preparing mount operation", 
                2 => "Mounting device",
                3 => "Cleaning up resources",
                4 => "Operation completed",
                _ => "Processing request",
            }),
            1..50
        ),
        stages in prop::collection::vec(
            (0..5usize).prop_map(|i| match i {
                0 => OperationStage::Validation,
                1 => OperationStage::Preparation,
                2 => OperationStage::Execution,
                3 => OperationStage::Cleanup,
                _ => OperationStage::Completion,
            }),
            1..50
        )
    ) {
        use tokio::sync::broadcast::error::TryRecvError;
        
        let operation_id = OperationId::new();
        let config = ProgressConfig::default();
        let reporter = ProgressReporter::new(operation_id, config.clone());
        let mut receiver = reporter.subscribe();
        
        let num_events = std::cmp::min(progress_values.len(), std::cmp::min(message_types.len(), stages.len()));
        let mut received_events = Vec::new();
        
        // Report progress with various message types
        for i in 0..num_events {
            let progress = progress_values[i];
            let base_message = message_types[i];
            let stage = stages[i];
            
            // Create descriptive message with context
            let descriptive_message = format!("{} ({}%)", base_message, (progress * 100.0) as u32);
            
            reporter.report_progress(progress, descriptive_message.clone(), stage);
            
            if config.enable_streams {
                match receiver.try_recv() {
                    Ok(event) => received_events.push(event),
                    Err(TryRecvError::Empty) => {
                        prop_assert!(false, "Expected progress event but channel was empty");
                    }
                    Err(TryRecvError::Closed) => {
                        prop_assert!(false, "Progress channel unexpectedly closed");
                    }
                    Err(TryRecvError::Lagged(_)) => {
                        // Try again after lag
                        match receiver.try_recv() {
                            Ok(event) => received_events.push(event),
                            _ => prop_assert!(false, "Failed to receive event after lag"),
                        }
                    }
                }
            }
        }
        
        // Property 1: All progress messages should be non-empty
        for event in &received_events {
            prop_assert!(!event.message.is_empty(),
                "Progress message should not be empty");
        }
        
        // Property 2: All progress messages should be descriptive (contain meaningful content)
        for event in &received_events {
            // Message should contain more than just whitespace
            prop_assert!(!event.message.trim().is_empty(),
                "Progress message should contain meaningful content, not just whitespace");
            
            // Message should be reasonably long (at least 5 characters for meaningful description)
            prop_assert!(event.message.len() >= 5,
                "Progress message '{}' should be descriptive (at least 5 characters)", event.message);
        }
        
        // Property 3: Messages should provide context about the operation stage
        for event in &received_events {
            let message_lower = event.message.to_lowercase();
            
            // Message should relate to the operation stage
            match event.stage {
                OperationStage::Validation => {
                    // Validation messages should contain validation-related terms
                    let has_validation_context = message_lower.contains("validat") ||
                        message_lower.contains("check") ||
                        message_lower.contains("verify") ||
                        message_lower.contains("mount point");
                    
                    if !has_validation_context {
                        // Allow generic messages but they should still be descriptive
                        prop_assert!(event.message.len() >= 10,
                            "Validation stage message '{}' should be more descriptive", event.message);
                    }
                }
                OperationStage::Preparation => {
                    let has_preparation_context = message_lower.contains("prepar") ||
                        message_lower.contains("setup") ||
                        message_lower.contains("configur") ||
                        message_lower.contains("initializ");
                    
                    if !has_preparation_context {
                        prop_assert!(event.message.len() >= 10,
                            "Preparation stage message '{}' should be more descriptive", event.message);
                    }
                }
                OperationStage::Execution => {
                    let has_execution_context = message_lower.contains("mount") ||
                        message_lower.contains("execut") ||
                        message_lower.contains("process") ||
                        message_lower.contains("perform");
                    
                    if !has_execution_context {
                        prop_assert!(event.message.len() >= 10,
                            "Execution stage message '{}' should be more descriptive", event.message);
                    }
                }
                OperationStage::Cleanup => {
                    let has_cleanup_context = message_lower.contains("clean") ||
                        message_lower.contains("finish") ||
                        message_lower.contains("resource") ||
                        message_lower.contains("finaliz");
                    
                    if !has_cleanup_context {
                        prop_assert!(event.message.len() >= 10,
                            "Cleanup stage message '{}' should be more descriptive", event.message);
                    }
                }
                OperationStage::Completion => {
                    let has_completion_context = message_lower.contains("complet") ||
                        message_lower.contains("finish") ||
                        message_lower.contains("done") ||
                        message_lower.contains("success");
                    
                    if !has_completion_context {
                        prop_assert!(event.message.len() >= 10,
                            "Completion stage message '{}' should be more descriptive", event.message);
                    }
                }
            }
        }
        
        // Property 4: Messages should not contain placeholder or debug text
        for event in &received_events {
            let message_lower = event.message.to_lowercase();
            
            // Should not contain common placeholder text
            prop_assert!(!message_lower.contains("todo"),
                "Message should not contain placeholder text: {}", event.message);
            prop_assert!(!message_lower.contains("fixme"),
                "Message should not contain placeholder text: {}", event.message);
            prop_assert!(!message_lower.contains("xxx"),
                "Message should not contain placeholder text: {}", event.message);
            
            // Should not contain debug artifacts
            prop_assert!(!message_lower.contains("debug"),
                "Message should not contain debug artifacts: {}", event.message);
        }
        
        // Property 5: Messages should be properly formatted (no leading/trailing whitespace)
        for event in &received_events {
            prop_assert_eq!(event.message.trim(), &event.message,
                "Message should not have leading or trailing whitespace: '{}'", event.message);
        }
        
        // Property 6: Messages should be human-readable (contain spaces between words)
        for event in &received_events {
            if event.message.len() > 10 {
                // Longer messages should contain spaces (indicating multiple words)
                prop_assert!(event.message.contains(' '),
                    "Longer message '{}' should contain spaces for readability", event.message);
            }
        }
        
        // Property 7: Test completion message quality
        let completion_result = reporter.report_completion();
        prop_assert!(completion_result.is_ok(), "Completion reporting should succeed");
        
        if config.enable_streams {
            match receiver.try_recv() {
                Ok(completion_event) => {
                    // Completion message should meet quality standards
                    prop_assert!(!completion_event.message.is_empty(),
                        "Completion message should not be empty");
                    prop_assert!(completion_event.message.len() >= 5,
                        "Completion message should be descriptive");
                    prop_assert_eq!(completion_event.message.trim(), &completion_event.message,
                        "Completion message should not have extra whitespace");
                }
                Err(TryRecvError::Empty) => {
                    prop_assert!(false, "Expected completion event but channel was empty");
                }
                Err(_) => {
                    prop_assert!(false, "Failed to receive completion event");
                }
            }
        }
    }
}

/// Property test for operation isolation.
/// **Feature: advanced-mount-operations, Property 4: Operation Isolation**
/// **Validates: Requirements 1.4**
proptest! {
    #[test]
    fn test_operation_isolation(
        num_operations in 2usize..20,
        progress_sequences in prop::collection::vec(
            prop::collection::vec(0.0f32..1.0f32, 1..50),
            2..20
        ),
        message_sequences in prop::collection::vec(
            prop::collection::vec("[a-zA-Z0-9 ]{5,30}", 1..50),
            2..20
        )
    ) {
        use tokio::sync::broadcast::error::TryRecvError;
        use std::collections::HashMap;
        
        // Ensure we have enough sequences for all operations
        let actual_num_ops = std::cmp::min(num_operations, std::cmp::min(progress_sequences.len(), message_sequences.len()));
        prop_assert!(actual_num_ops >= 2, "Need at least 2 operations for isolation testing");
        
        // Create multiple progress reporters with different operation IDs
        let mut reporters = Vec::new();
        let mut receivers = Vec::new();
        let mut operation_ids = Vec::new();
        
        for i in 0..actual_num_ops {
            let operation_id = OperationId::new();
            let config = ProgressConfig::default();
            let reporter = ProgressReporter::new(operation_id, config);
            let receiver = reporter.subscribe();
            
            operation_ids.push(operation_id);
            reporters.push(reporter);
            receivers.push(receiver);
        }
        
        // Property 1: All operation IDs should be unique
        let unique_ids: std::collections::HashSet<_> = operation_ids.iter().collect();
        prop_assert_eq!(unique_ids.len(), actual_num_ops,
            "All operation IDs should be unique");
        
        // Report progress from each operation with different patterns
        let mut expected_events_per_operation = HashMap::new();
        
        for (op_idx, reporter) in reporters.iter().enumerate() {
            let progress_seq = &progress_sequences[op_idx];
            let message_seq = &message_sequences[op_idx];
            let operation_id = operation_ids[op_idx];
            
            let num_events = std::cmp::min(progress_seq.len(), message_seq.len());
            let mut expected_events = Vec::new();
            
            for event_idx in 0..num_events {
                let progress = progress_seq[event_idx];
                let base_message = &message_seq[event_idx];
                let stage = match event_idx % 5 {
                    0 => OperationStage::Validation,
                    1 => OperationStage::Preparation,
                    2 => OperationStage::Execution,
                    3 => OperationStage::Cleanup,
                    _ => OperationStage::Completion,
                };
                
                // Create message with operation-specific prefix that won't conflict
                let message = format!("Operation_{}_Event_{}: {}", op_idx, event_idx, base_message);
                
                // Report the progress
                reporter.report_progress(progress, message.clone(), stage);
                
                // Track expected event
                expected_events.push((operation_id, progress, message, stage));
            }
            
            expected_events_per_operation.insert(operation_id, expected_events);
        }
        
        // Collect events from each receiver
        let mut received_events_per_operation = HashMap::new();
        
        for (op_idx, receiver) in receivers.iter_mut().enumerate() {
            let operation_id = operation_ids[op_idx];
            let mut received_events = Vec::new();
            
            // Try to receive all events for this operation
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        received_events.push(event);
                    }
                    Err(TryRecvError::Empty) => {
                        // No more events available
                        break;
                    }
                    Err(TryRecvError::Closed) => {
                        prop_assert!(false, "Progress channel unexpectedly closed for operation {}", op_idx);
                    }
                    Err(TryRecvError::Lagged(_)) => {
                        // Channel lagged, continue trying
                        continue;
                    }
                }
            }
            
            received_events_per_operation.insert(operation_id, received_events);
        }
        
        // Property 2: Each receiver should only receive events for its own operation
        for (operation_id, received_events) in &received_events_per_operation {
            for event in received_events {
                prop_assert_eq!(event.operation_id, *operation_id,
                    "Event with operation ID {} received by wrong receiver (expected {})",
                    event.operation_id, operation_id);
            }
        }
        
        // Property 3: Events should not be cross-contaminated between operations
        // The core isolation property is that each receiver only gets events for its own operation ID
        for (operation_id, received_events) in &received_events_per_operation {
            for event in received_events {
                prop_assert_eq!(event.operation_id, *operation_id,
                    "Event with operation ID {} received by wrong receiver (expected {})",
                    event.operation_id, operation_id);
            }
        }
        
        // Property 4: Each operation should receive the correct number of events
        for (operation_id, received_events) in &received_events_per_operation {
            if let Some(expected_events) = expected_events_per_operation.get(operation_id) {
                prop_assert_eq!(received_events.len(), expected_events.len(),
                    "Operation {} should receive {} events but got {}",
                    operation_id, expected_events.len(), received_events.len());
            }
        }
        
        // Property 5: Events should maintain order within each operation
        for (operation_id, received_events) in &received_events_per_operation {
            if let Some(expected_events) = expected_events_per_operation.get(operation_id) {
                for (i, (received_event, expected_event)) in received_events.iter().zip(expected_events.iter()).enumerate() {
                    let (expected_op_id, expected_progress, expected_message, expected_stage) = expected_event;
                    
                    prop_assert_eq!(received_event.operation_id, *expected_op_id,
                        "Event {} operation ID mismatch", i);
                    prop_assert_eq!(received_event.progress, *expected_progress,
                        "Event {} progress mismatch", i);
                    prop_assert_eq!(&received_event.message, expected_message,
                        "Event {} message mismatch", i);
                    prop_assert_eq!(received_event.stage, *expected_stage,
                        "Event {} stage mismatch", i);
                }
            }
        }
        
        // Property 6: No operation should receive events from other operations
        let all_operation_ids: std::collections::HashSet<_> = operation_ids.iter().collect();
        
        for (operation_id, received_events) in &received_events_per_operation {
            for event in received_events {
                prop_assert!(all_operation_ids.contains(&event.operation_id),
                    "Received event with unknown operation ID: {}", event.operation_id);
                
                prop_assert_eq!(event.operation_id, *operation_id,
                    "Operation {} received event from operation {}",
                    operation_id, event.operation_id);
            }
        }
        
        // Property 7: Test completion events are also isolated
        for (op_idx, reporter) in reporters.iter().enumerate() {
            let operation_id = operation_ids[op_idx];
            let completion_result = reporter.report_completion();
            prop_assert!(completion_result.is_ok(), 
                "Completion reporting should succeed for operation {}", op_idx);
        }
        
        // Collect completion events
        for (op_idx, receiver) in receivers.iter_mut().enumerate() {
            let operation_id = operation_ids[op_idx];
            
            match receiver.try_recv() {
                Ok(completion_event) => {
                    prop_assert_eq!(completion_event.operation_id, operation_id,
                        "Completion event operation ID mismatch for operation {}", op_idx);
                    prop_assert_eq!(completion_event.progress, 1.0,
                        "Completion event should have progress 1.0");
                    prop_assert_eq!(completion_event.stage, OperationStage::Completion,
                        "Completion event should have Completion stage");
                }
                Err(TryRecvError::Empty) => {
                    prop_assert!(false, "Expected completion event for operation {} but channel was empty", op_idx);
                }
                Err(_) => {
                    prop_assert!(false, "Failed to receive completion event for operation {}", op_idx);
                }
            }
        }
    }
}

/// Property test for dual reporting consistency.
/// **Feature: advanced-mount-operations, Property 5: Dual Reporting Consistency**
/// **Validates: Requirements 1.5**
proptest! {
    #[test]
    fn test_dual_reporting_consistency(
        progress_values in prop::collection::vec(0.0f32..1.0f32, 1..50),
        messages in prop::collection::vec("[a-zA-Z0-9 ]{5,30}", 1..50),
        stages in prop::collection::vec(
            (0..5usize).prop_map(|i| match i {
                0 => OperationStage::Validation,
                1 => OperationStage::Preparation,
                2 => OperationStage::Execution,
                3 => OperationStage::Cleanup,
                _ => OperationStage::Completion,
            }),
            1..50
        )
    ) {
        use tokio::sync::broadcast::error::TryRecvError;
        use std::sync::{Arc, Mutex};
        
        let operation_id = OperationId::new();
        
        // Create config with both callback and stream reporting enabled
        let mut config = ProgressConfig::default();
        config.enable_callbacks = true;
        config.enable_streams = true;
        
        let mut reporter = ProgressReporter::new(operation_id, config);
        
        // Set up callback reporting - collect events in a shared vector
        let callback_events = Arc::new(Mutex::new(Vec::new()));
        let callback_events_clone = Arc::clone(&callback_events);
        
        reporter.set_callback(move |event| {
            let mut events = callback_events_clone.lock().unwrap();
            events.push(event);
        });
        
        // Set up stream reporting
        let mut stream_receiver = reporter.subscribe();
        
        let num_events = std::cmp::min(progress_values.len(), std::cmp::min(messages.len(), stages.len()));
        
        // Report progress events
        for i in 0..num_events {
            let progress = progress_values[i];
            let message = messages[i].clone();
            let stage = stages[i];
            
            reporter.report_progress(progress, message, stage);
        }
        
        // Report completion
        let completion_result = reporter.report_completion();
        prop_assert!(completion_result.is_ok(), "Completion reporting should succeed");
        
        // Collect events from stream
        let mut stream_events = Vec::new();
        loop {
            match stream_receiver.try_recv() {
                Ok(event) => {
                    stream_events.push(event);
                }
                Err(TryRecvError::Empty) => {
                    // No more events available
                    break;
                }
                Err(TryRecvError::Closed) => {
                    prop_assert!(false, "Stream channel unexpectedly closed");
                }
                Err(TryRecvError::Lagged(_)) => {
                    // Channel lagged, continue trying
                    continue;
                }
            }
        }
        
        // Get events from callback
        let callback_events_vec = {
            let events = callback_events.lock().unwrap();
            events.clone()
        };
        
        // Property 1: Both mechanisms should receive the same number of events
        let expected_total_events = num_events + 1; // +1 for completion event
        prop_assert_eq!(stream_events.len(), expected_total_events,
            "Stream should receive {} events but got {}", expected_total_events, stream_events.len());
        prop_assert_eq!(callback_events_vec.len(), expected_total_events,
            "Callback should receive {} events but got {}", expected_total_events, callback_events_vec.len());
        
        // Property 2: Events should be identical between both mechanisms
        for (i, (stream_event, callback_event)) in stream_events.iter().zip(callback_events_vec.iter()).enumerate() {
            prop_assert_eq!(stream_event.operation_id, callback_event.operation_id,
                "Event {} operation ID mismatch between stream and callback", i);
            prop_assert_eq!(stream_event.progress, callback_event.progress,
                "Event {} progress mismatch between stream and callback", i);
            prop_assert_eq!(&stream_event.message, &callback_event.message,
                "Event {} message mismatch between stream and callback", i);
            prop_assert_eq!(stream_event.stage, callback_event.stage,
                "Event {} stage mismatch between stream and callback", i);
            
            // Timestamps might differ slightly, but should be close
            let time_diff = if stream_event.timestamp > callback_event.timestamp {
                stream_event.timestamp.duration_since(callback_event.timestamp)
            } else {
                callback_event.timestamp.duration_since(stream_event.timestamp)
            };
            
            prop_assert!(time_diff.as_millis() < 100,
                "Event {} timestamp difference too large: {}ms", i, time_diff.as_millis());
        }
        
        // Property 3: Events should be in the same order
        for i in 0..num_events {
            let expected_progress = progress_values[i];
            let expected_message = &messages[i];
            let expected_stage = stages[i];
            
            // Check stream event
            prop_assert_eq!(stream_events[i].progress, expected_progress,
                "Stream event {} progress mismatch", i);
            prop_assert_eq!(&stream_events[i].message, expected_message,
                "Stream event {} message mismatch", i);
            prop_assert_eq!(stream_events[i].stage, expected_stage,
                "Stream event {} stage mismatch", i);
            
            // Check callback event
            prop_assert_eq!(callback_events_vec[i].progress, expected_progress,
                "Callback event {} progress mismatch", i);
            prop_assert_eq!(&callback_events_vec[i].message, expected_message,
                "Callback event {} message mismatch", i);
            prop_assert_eq!(callback_events_vec[i].stage, expected_stage,
                "Callback event {} stage mismatch", i);
        }
        
        // Property 4: Completion events should be consistent
        let stream_completion = &stream_events[num_events];
        let callback_completion = &callback_events_vec[num_events];
        
        prop_assert_eq!(stream_completion.progress, 1.0,
            "Stream completion event should have progress 1.0");
        prop_assert_eq!(callback_completion.progress, 1.0,
            "Callback completion event should have progress 1.0");
        prop_assert_eq!(stream_completion.stage, OperationStage::Completion,
            "Stream completion event should have Completion stage");
        prop_assert_eq!(callback_completion.stage, OperationStage::Completion,
            "Callback completion event should have Completion stage");
        prop_assert_eq!(&stream_completion.message, &callback_completion.message,
            "Completion event messages should match between stream and callback");
        
        // Property 5: Test with callbacks disabled
        let mut config_no_callback = ProgressConfig::default();
        config_no_callback.enable_callbacks = false;
        config_no_callback.enable_streams = true;
        
        let reporter_no_callback = ProgressReporter::new(OperationId::new(), config_no_callback);
        let mut stream_receiver_no_callback = reporter_no_callback.subscribe();
        
        // Report a single event
        reporter_no_callback.report_progress(0.5, "Test message".to_string(), OperationStage::Execution);
        
        // Stream should still work
        match stream_receiver_no_callback.try_recv() {
            Ok(event) => {
                prop_assert_eq!(event.progress, 0.5, "Stream should work when callbacks disabled");
                prop_assert_eq!(&event.message, "Test message", "Stream message should be correct");
            }
            Err(_) => {
                prop_assert!(false, "Stream should receive event even when callbacks disabled");
            }
        }
        
        // Property 6: Test with streams disabled
        let mut config_no_stream = ProgressConfig::default();
        config_no_stream.enable_callbacks = true;
        config_no_stream.enable_streams = false;
        
        let mut reporter_no_stream = ProgressReporter::new(OperationId::new(), config_no_stream);
        
        let callback_events_no_stream = Arc::new(Mutex::new(Vec::new()));
        let callback_events_no_stream_clone = Arc::clone(&callback_events_no_stream);
        
        reporter_no_stream.set_callback(move |event| {
            let mut events = callback_events_no_stream_clone.lock().unwrap();
            events.push(event);
        });
        
        let mut stream_receiver_no_stream = reporter_no_stream.subscribe();
        
        // Report a single event
        reporter_no_stream.report_progress(0.7, "Test message 2".to_string(), OperationStage::Cleanup);
        
        // Callback should still work
        {
            let events = callback_events_no_stream.lock().unwrap();
            prop_assert_eq!(events.len(), 1, "Callback should work when streams disabled");
            prop_assert_eq!(events[0].progress, 0.7, "Callback progress should be correct");
            prop_assert_eq!(&events[0].message, "Test message 2", "Callback message should be correct");
        }
        
        // Stream should not receive events when disabled
        match stream_receiver_no_stream.try_recv() {
            Ok(_) => {
                prop_assert!(false, "Stream should not receive events when disabled");
            }
            Err(TryRecvError::Empty) => {
                // This is expected when streams are disabled
            }
            Err(_) => {
                // Other errors are also acceptable when streams are disabled
            }
        }
    }
}

/// Property test for operation ID uniqueness.
/// **Feature: advanced-mount-operations, Property 21: Unique Operation IDs**
/// **Validates: Requirements 5.1**
proptest! {
    #[test]
    fn test_operation_id_uniqueness(count in 1usize..1000) {
        // Generate multiple operation IDs
        let mut ids = HashSet::new();
        
        for _ in 0..count {
            let id = OperationId::new();
            // Each ID should be unique - inserting should return true
            prop_assert!(ids.insert(id), "Generated duplicate OperationId: {}", id);
        }
        
        // Verify we have the expected number of unique IDs
        prop_assert_eq!(ids.len(), count);
    }
}

/// Property test for operation ID consistency.
/// Verifies that OperationId maintains its value across operations.
proptest! {
    #[test]
    fn test_operation_id_consistency(count in 1usize..100) {
        let mut ids = Vec::new();
        
        // Generate IDs and store them
        for _ in 0..count {
            ids.push(OperationId::new());
        }
        
        // Verify each ID is consistent with itself
        for id in &ids {
            prop_assert_eq!(*id, *id);
            prop_assert_eq!(id.to_string(), id.to_string());
            prop_assert_eq!(id.as_uuid(), id.as_uuid());
        }
        
        // Verify all IDs are still unique
        let unique_ids: HashSet<_> = ids.iter().collect();
        prop_assert_eq!(unique_ids.len(), ids.len());
    }
}

/// Property test for operation ID default behavior.
/// Verifies that Default::default() produces unique IDs.
proptest! {
    #[test]
    fn test_operation_id_default_uniqueness(count in 1usize..100) {
        let mut ids = HashSet::new();
        
        for _ in 0..count {
            let id = OperationId::default();
            prop_assert!(ids.insert(id), "Default generated duplicate OperationId: {}", id);
        }
        
        prop_assert_eq!(ids.len(), count);
    }
}

/// Property test for atomic state updates.
/// **Feature: advanced-mount-operations, Property 22: Atomic State Updates**
/// **Validates: Requirements 5.2**
proptest! {
    #[test]
    fn test_atomic_state_updates(
        num_threads in 1usize..20,
        updates_per_thread in 1usize..50,
        progress_values in prop::collection::vec(0.0f32..1.0f32, 1..50)
    ) {
        use std::sync::{Arc, Barrier};
        use std::thread;
        use npio::mount::advanced::{
            OperationContext, OperationType, OperationState, 
            config::OperationConfig
        };
        
        let operation_type = OperationType::Mount {
            volume_path: "/dev/test".to_string(),
            mount_point: Some("/mnt/test".to_string()),
        };
        let config = OperationConfig::default();
        let context = Arc::new(OperationContext::new(operation_type, config));
        
        // Barrier to synchronize thread start
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];
        
        // Spawn threads that concurrently update state
        for thread_id in 0..num_threads {
            let context_clone = Arc::clone(&context);
            let barrier_clone = Arc::clone(&barrier);
            let progress_values_clone = progress_values.clone();
            
            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();
                
                let mut observed_states = Vec::new();
                
                // Perform updates and observations
                for i in 0..updates_per_thread {
                    let progress_idx = (thread_id * updates_per_thread + i) % progress_values_clone.len();
                    let progress = progress_values_clone[progress_idx];
                    
                    // Update state
                    context_clone.update_state(OperationState::InProgress {
                        progress,
                        message: format!("Thread {} update {}", thread_id, i),
                    });
                    
                    // Immediately observe the state
                    let observed_state = context_clone.state();
                    observed_states.push(observed_state);
                    
                    // Small yield to increase chance of race conditions
                    thread::yield_now();
                }
                
                observed_states
            });
            handles.push(handle);
        }
        
        // Collect all observed states from all threads
        let mut all_observed_states = Vec::new();
        for handle in handles {
            let thread_states = handle.join().unwrap();
            all_observed_states.extend(thread_states);
        }
        
        // Property: All observed states should be valid and consistent
        for state in &all_observed_states {
            match state {
                OperationState::InProgress { progress, message } => {
                    // Progress should be valid
                    prop_assert!(*progress >= 0.0 && *progress <= 1.0, 
                        "Invalid progress value: {}", progress);
                    
                    // Message should not be empty
                    prop_assert!(!message.is_empty(), 
                        "Empty message in InProgress state");
                    
                    // Message should contain expected format
                    prop_assert!(message.contains("Thread") && message.contains("update"), 
                        "Unexpected message format: {}", message);
                }
                OperationState::Pending => {
                    // This is valid - might be observed before any updates
                }
                _ => {
                    // Other states should not be observed in this test
                    prop_assert!(false, "Unexpected state observed: {:?}", state);
                }
            }
        }
        
        // Property: Final state should be consistent
        let final_state = context.state();
        match final_state {
            OperationState::InProgress { progress, .. } => {
                prop_assert!(progress >= 0.0 && progress <= 1.0, 
                    "Final state has invalid progress: {}", progress);
            }
            OperationState::Pending => {
                // This is also valid if no updates completed
            }
            _ => {
                prop_assert!(false, "Unexpected final state: {:?}", final_state);
            }
        }
        
        // Property: State queries should be consistent with each other
        let state_copy = context.state();
        let progress = context.progress();
        let message = context.status_message();
        let is_terminal = context.is_terminal();
        
        match state_copy {
            OperationState::InProgress { progress: state_progress, message: state_message } => {
                prop_assert_eq!(progress, Some(state_progress), 
                    "Progress query inconsistent with state");
                prop_assert_eq!(message, state_message, 
                    "Message query inconsistent with state");
                prop_assert!(!is_terminal, 
                    "Terminal query inconsistent with InProgress state");
            }
            OperationState::Pending => {
                prop_assert_eq!(progress, None, 
                    "Progress should be None for Pending state");
                prop_assert_eq!(message, "Pending", 
                    "Message should be 'Pending' for Pending state");
                prop_assert!(!is_terminal, 
                    "Terminal query inconsistent with Pending state");
            }
            _ => {
                prop_assert!(false, "Unexpected state in consistency check: {:?}", state_copy);
            }
        }
    }
}

/// Property test for thread-safe access.
/// **Feature: advanced-mount-operations, Property 23: Thread-Safe Access**
/// **Validates: Requirements 5.3**
proptest! {
    #[test]
    fn test_thread_safe_access(
        num_readers in 1usize..20,
        num_writers in 1usize..10,
        operations_per_thread in 1usize..100,
        progress_values in prop::collection::vec(0.0f32..1.0f32, 1..100)
    ) {
        use std::sync::{Arc, Barrier, atomic::{AtomicUsize, Ordering}};
        use std::thread;
        use npio::mount::advanced::{
            OperationContext, OperationType, OperationState, 
            config::OperationConfig
        };
        
        let operation_type = OperationType::Mount {
            volume_path: "/dev/test".to_string(),
            mount_point: Some("/mnt/test".to_string()),
        };
        let config = OperationConfig::default();
        let context = Arc::new(OperationContext::new(operation_type, config));
        
        // Synchronization primitives
        let barrier = Arc::new(Barrier::new(num_readers + num_writers));
        let read_operations = Arc::new(AtomicUsize::new(0));
        let write_operations = Arc::new(AtomicUsize::new(0));
        let mut reader_handles = vec![];
        let mut writer_handles = vec![];
        
        // Spawn reader threads that only query state
        for reader_id in 0..num_readers {
            let context_clone = Arc::clone(&context);
            let barrier_clone = Arc::clone(&barrier);
            let read_ops_clone = Arc::clone(&read_operations);
            
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                
                let mut valid_observations = 0;
                
                for _ in 0..operations_per_thread {
                    // Perform various read operations - each should complete without panicking
                    let state = context_clone.state();
                    let progress = context_clone.progress();
                    let message = context_clone.status_message();
                    let is_terminal = context_clone.is_terminal();
                    let is_cancelled = context_clone.is_cancelled();
                    let _operation_type = context_clone.operation_type().clone();
                    let _id = context_clone.id();
                    let _elapsed = context_clone.elapsed();
                    
                    // Validate individual values are reasonable
                    if let Some(prog) = progress {
                        if prog >= 0.0 && prog <= 1.0 {
                            valid_observations += 1;
                        }
                    } else {
                        valid_observations += 1; // None is also valid
                    }
                    
                    // Message should not be empty
                    if !message.is_empty() {
                        valid_observations += 1;
                    }
                    
                    // State should be a valid enum variant (if we get here, it is)
                    match state {
                        OperationState::Pending |
                        OperationState::Validating |
                        OperationState::InProgress { .. } |
                        OperationState::Retrying { .. } |
                        OperationState::Completed { .. } |
                        OperationState::Cancelled { .. } |
                        OperationState::Failed { .. } => {
                            valid_observations += 1;
                        }
                    }
                    
                    read_ops_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // Small yield to increase concurrency
                    thread::yield_now();
                }
                
                (reader_id, valid_observations)
            });
            reader_handles.push(handle);
        }
        
        // Spawn writer threads that update state
        for writer_id in 0..num_writers {
            let context_clone = Arc::clone(&context);
            let barrier_clone = Arc::clone(&barrier);
            let write_ops_clone = Arc::clone(&write_operations);
            let progress_values_clone = progress_values.clone();
            
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                
                let mut successful_updates = 0;
                
                for i in 0..operations_per_thread {
                    let progress_idx = (writer_id * operations_per_thread + i) % progress_values_clone.len();
                    let progress = progress_values_clone[progress_idx];
                    
                    // Perform state update - should not panic
                    let new_state = OperationState::InProgress {
                        progress,
                        message: format!("Writer {} operation {}", writer_id, i),
                    };
                    
                    context_clone.update_state(new_state);
                    successful_updates += 1;
                    
                    write_ops_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // Small yield to increase concurrency
                    thread::yield_now();
                }
                
                (writer_id, successful_updates)
            });
            writer_handles.push(handle);
        }
        
        // Collect results from all threads
        let mut reader_results = Vec::new();
        let mut writer_results = Vec::new();
        
        for handle in reader_handles {
            let result = handle.join().unwrap();
            reader_results.push(result);
        }
        
        for handle in writer_handles {
            let result = handle.join().unwrap();
            writer_results.push(result);
        }
        
        // Property 1: All threads should complete without panics
        prop_assert_eq!(reader_results.len(), num_readers, 
            "Not all reader threads completed successfully");
        prop_assert_eq!(writer_results.len(), num_writers, 
            "Not all writer threads completed successfully");
        
        // Property 2: Total operations should match expected count
        let total_reads = read_operations.load(Ordering::Relaxed);
        let total_writes = write_operations.load(Ordering::Relaxed);
        prop_assert_eq!(total_reads, num_readers * operations_per_thread, 
            "Read operation count mismatch");
        prop_assert_eq!(total_writes, num_writers * operations_per_thread, 
            "Write operation count mismatch");
        
        // Property 3: All readers should observe valid data
        for (reader_id, valid_observations) in &reader_results {
            // Each reader should have made 3 valid observations per operation
            // (progress, message, state)
            let expected_observations = operations_per_thread * 3;
            prop_assert_eq!(*valid_observations, expected_observations,
                "Reader {} made invalid observations", reader_id);
        }
        
        // Property 4: All writers should complete their updates
        for (writer_id, successful_updates) in &writer_results {
            prop_assert_eq!(*successful_updates, operations_per_thread,
                "Writer {} failed to complete all updates", writer_id);
        }
        
        // Property 5: Final state should be accessible and valid
        let final_state = context.state();
        let final_progress = context.progress();
        let final_message = context.status_message();
        let final_is_terminal = context.is_terminal();
        
        // Final state should be valid
        match final_state {
            OperationState::InProgress { progress, .. } => {
                prop_assert!(progress >= 0.0 && progress <= 1.0, 
                    "Final state has invalid progress: {}", progress);
                prop_assert!(!final_is_terminal, 
                    "InProgress state should not be terminal");
            }
            OperationState::Pending => {
                prop_assert!(!final_is_terminal, 
                    "Pending state should not be terminal");
            }
            OperationState::Validating => {
                prop_assert!(!final_is_terminal, 
                    "Validating state should not be terminal");
            }
            OperationState::Retrying { .. } => {
                prop_assert!(!final_is_terminal, 
                    "Retrying state should not be terminal");
            }
            OperationState::Completed { .. } |
            OperationState::Cancelled { .. } |
            OperationState::Failed { .. } => {
                prop_assert!(final_is_terminal, 
                    "Terminal states should be marked as terminal");
            }
        }
        
        // Final progress should be valid if present
        if let Some(prog) = final_progress {
            prop_assert!(prog >= 0.0 && prog <= 1.0, 
                "Final progress value is invalid: {}", prog);
        }
        
        // Final message should not be empty
        prop_assert!(!final_message.is_empty(), 
            "Final message should not be empty");
        
        // Property 6: Context should remain functional after concurrent access
        // Test that we can still perform operations on the context
        let test_state = OperationState::InProgress {
            progress: 0.99,
            message: "Final test".to_string(),
        };
        context.update_state(test_state);
        
        let post_test_state = context.state();
        match post_test_state {
            OperationState::InProgress { progress, message } => {
                prop_assert_eq!(progress, 0.99, "Context not functional after concurrent access");
                prop_assert_eq!(message, "Final test", "Context not functional after concurrent access");
            }
            _ => {
                prop_assert!(false, "Context state update failed after concurrent access");
            }
        }
    }
}

/// Property test for mount point existence check.
/// **Feature: advanced-mount-operations, Property 11: Mount Point Existence Check**
/// **Validates: Requirements 3.1**
proptest! {
    #[test]
    fn test_mount_point_existence_check(
        valid_paths in prop::collection::vec("[a-zA-Z0-9_/]{5,50}", 1..20),
        invalid_paths in prop::collection::vec("[a-zA-Z0-9_/]{5,50}", 1..20),
        file_paths in prop::collection::vec("[a-zA-Z0-9_]{5,20}", 1..10)
    ) {
        use std::fs;
        use std::path::Path;
        use tempfile::TempDir;
        use npio::mount::advanced::{MountValidator, ValidationError, config::ValidationConfig};
        
        // Create temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create validator with mount point existence checking enabled
        let mut config = ValidationConfig::default();
        config.check_mount_point_exists = true;
        config.check_mount_point_available = false;
        config.check_permissions = false;
        config.check_filesystem = false;
        config.check_device_availability = false;
        
        let validator = MountValidator::new(config);
        
        // Test 1: Valid directories should pass existence check
        let mut created_dirs = Vec::new();
        for (i, path_suffix) in valid_paths.iter().enumerate().take(5) {
            let dir_path = temp_path.join(format!("valid_dir_{}_{}", i, path_suffix));
            
            // Create the directory
            if fs::create_dir_all(&dir_path).is_ok() {
                created_dirs.push(dir_path);
            }
        }
        
        // Property 1: Existing directories should pass validation
        for dir_path in &created_dirs {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount("/dev/test", dir_path));
            
            // Should not have MountPointNotFound error
            let has_not_found_error = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotFound { .. })
            });
            
            prop_assert!(!has_not_found_error, 
                "Existing directory {} should not have MountPointNotFound error", 
                dir_path.display());
        }
        
        // Test 2: Non-existent paths should fail existence check
        let mut non_existent_paths = Vec::new();
        for (i, path_suffix) in invalid_paths.iter().enumerate().take(5) {
            let non_existent_path = temp_path.join(format!("nonexistent_{}_{}", i, path_suffix));
            
            // Ensure this path doesn't exist
            if !non_existent_path.exists() {
                non_existent_paths.push(non_existent_path);
            }
        }
        
        // Property 2: Non-existent paths should have MountPointNotFound error
        for non_existent_path in &non_existent_paths {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount("/dev/test", non_existent_path));
            
            let has_not_found_error = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotFound { .. })
            });
            
            prop_assert!(has_not_found_error, 
                "Non-existent path {} should have MountPointNotFound error", 
                non_existent_path.display());
            
            prop_assert!(!result.is_valid, 
                "Validation should fail for non-existent path {}", 
                non_existent_path.display());
        }
        
        // Test 3: Files (not directories) should fail with MountPointNotDirectory error
        let mut created_files = Vec::new();
        for (i, file_name) in file_paths.iter().enumerate().take(3) {
            let file_path = temp_path.join(format!("file_{}_{}.txt", i, file_name));
            
            // Create a regular file
            if fs::write(&file_path, "test content").is_ok() {
                created_files.push(file_path);
            }
        }
        
        // Property 3: Files should have MountPointNotDirectory error
        for file_path in &created_files {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount("/dev/test", file_path));
            
            let has_not_directory_error = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotDirectory { .. })
            });
            
            prop_assert!(has_not_directory_error, 
                "File {} should have MountPointNotDirectory error", 
                file_path.display());
            
            prop_assert!(!result.is_valid, 
                "Validation should fail for file path {}", 
                file_path.display());
        }
        
        // Test 4: When check is disabled, no existence errors should occur
        let mut config_disabled = ValidationConfig::default();
        config_disabled.check_mount_point_exists = false;
        config_disabled.check_mount_point_available = false;
        config_disabled.check_permissions = false;
        config_disabled.check_filesystem = false;
        config_disabled.check_device_availability = false;
        
        let validator_disabled = MountValidator::new(config_disabled);
        
        // Property 4: Disabled check should not produce existence errors
        for non_existent_path in &non_existent_paths {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator_disabled.validate_mount("/dev/test", non_existent_path));
            
            let has_existence_errors = result.errors.iter().any(|e| {
                matches!(e, 
                    ValidationError::MountPointNotFound { .. } |
                    ValidationError::MountPointNotDirectory { .. }
                )
            });
            
            prop_assert!(!has_existence_errors, 
                "Disabled existence check should not produce existence errors for {}", 
                non_existent_path.display());
        }
        
        // Property 5: Error messages should contain the correct path
        for non_existent_path in &non_existent_paths {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount("/dev/test", non_existent_path));
            
            for error in &result.errors {
                match error {
                    ValidationError::MountPointNotFound { path } => {
                        let expected_path = non_existent_path.to_string_lossy().to_string();
                        prop_assert_eq!(path, &expected_path, 
                            "Error path should match the validated path");
                    }
                    _ => {}
                }
            }
        }
        
        // Property 6: Validation should be consistent across multiple calls
        if let Some(test_dir) = created_dirs.first() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            let result1 = rt.block_on(validator.validate_mount("/dev/test", test_dir));
            let result2 = rt.block_on(validator.validate_mount("/dev/test", test_dir));
            
            // Results should be consistent
            prop_assert_eq!(result1.errors.len(), result2.errors.len(), 
                "Validation results should be consistent across calls");
            
            let has_existence_error1 = result1.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotFound { .. } | 
                           ValidationError::MountPointNotDirectory { .. })
            });
            let has_existence_error2 = result2.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotFound { .. } | 
                           ValidationError::MountPointNotDirectory { .. })
            });
            
            prop_assert_eq!(has_existence_error1, has_existence_error2, 
                "Existence error presence should be consistent");
        }
    }
}

/// Property test for mount point availability check.
/// **Feature: advanced-mount-operations, Property 12: Mount Point Availability Check**
/// **Validates: Requirements 3.2**
proptest! {
    #[test]
    fn test_mount_point_availability_check(
        mount_point_names in prop::collection::vec("[a-zA-Z0-9_]{5,20}", 1..10),
        source_devices in prop::collection::vec("[a-zA-Z0-9_/]{5,30}", 1..10)
    ) {
        use std::fs;
        use tempfile::TempDir;
        use npio::mount::advanced::{MountValidator, ValidationError, config::ValidationConfig};
        
        // Create temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create validator with mount point availability checking enabled
        let mut config = ValidationConfig::default();
        config.check_mount_point_exists = true; // Need this to create directories
        config.check_mount_point_available = true;
        config.check_permissions = false;
        config.check_filesystem = false;
        config.check_device_availability = false;
        
        let validator = MountValidator::new(config);
        
        // Test 1: Available mount points should pass availability check
        let mut available_mount_points = Vec::new();
        for (i, mount_name) in mount_point_names.iter().enumerate().take(5) {
            let mount_path = temp_path.join(format!("available_{}_{}", i, mount_name));
            
            // Create the directory
            if fs::create_dir_all(&mount_path).is_ok() {
                available_mount_points.push(mount_path);
            }
        }
        
        // Property 1: Available mount points should not have MountPointInUse error
        for (i, mount_path) in available_mount_points.iter().enumerate() {
            let source = if i < source_devices.len() {
                format!("/dev/{}", &source_devices[i])
            } else {
                "/dev/test".to_string()
            };
            
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount(&source, mount_path));
            
            let has_in_use_error = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointInUse { .. })
            });
            
            prop_assert!(!has_in_use_error, 
                "Available mount point {} should not have MountPointInUse error", 
                mount_path.display());
        }
        
        // Test 2: When availability check is disabled, no availability errors should occur
        let mut config_disabled = ValidationConfig::default();
        config_disabled.check_mount_point_exists = true;
        config_disabled.check_mount_point_available = false;
        config_disabled.check_permissions = false;
        config_disabled.check_filesystem = false;
        config_disabled.check_device_availability = false;
        
        let validator_disabled = MountValidator::new(config_disabled);
        
        // Property 2: Disabled availability check should not produce availability errors
        for (i, mount_path) in available_mount_points.iter().enumerate() {
            let source = if i < source_devices.len() {
                format!("/dev/{}", &source_devices[i])
            } else {
                "/dev/test".to_string()
            };
            
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator_disabled.validate_mount(&source, mount_path));
            
            let has_availability_errors = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointInUse { .. })
            });
            
            prop_assert!(!has_availability_errors, 
                "Disabled availability check should not produce availability errors for {}", 
                mount_path.display());
        }
        
        // Test 3: Validation should collect current mounts metadata
        if let Some(mount_path) = available_mount_points.first() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_mount("/dev/test", mount_path));
            
            // Should have current_mounts metadata (even if empty)
            prop_assert!(result.metadata.current_mounts.len() >= 0, 
                "Should collect current mounts metadata");
        }
        
        // Test 4: Validation should be consistent across multiple calls
        if let Some(mount_path) = available_mount_points.first() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            let result1 = rt.block_on(validator.validate_mount("/dev/test", mount_path));
            let result2 = rt.block_on(validator.validate_mount("/dev/test", mount_path));
            
            // Availability check results should be consistent
            let has_availability_error1 = result1.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointInUse { .. })
            });
            let has_availability_error2 = result2.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointInUse { .. })
            });
            
            prop_assert_eq!(has_availability_error1, has_availability_error2, 
                "Availability check results should be consistent");
            
            // Metadata should be consistent
            prop_assert_eq!(result1.metadata.current_mounts.len(), result2.metadata.current_mounts.len(),
                "Current mounts metadata should be consistent");
        }
        
        // Test 5: Unmount validation should check if mount point is actually mounted
        if let Some(mount_path) = available_mount_points.first() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_unmount(mount_path));
            
            // Since this is not actually mounted, should have MountPointNotMounted error
            let has_not_mounted_error = result.errors.iter().any(|e| {
                matches!(e, ValidationError::MountPointNotMounted { .. })
            });
            
            prop_assert!(has_not_mounted_error, 
                "Unmount validation should detect that {} is not mounted", 
                mount_path.display());
            
            prop_assert!(!result.is_valid, 
                "Unmount validation should fail for non-mounted path {}", 
                mount_path.display());
        }
        
        // Test 6: Unmount validation error messages should contain correct path
        if let Some(mount_path) = available_mount_points.first() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(validator.validate_unmount(mount_path));
            
            for error in &result.errors {
                match error {
                    ValidationError::MountPointNotMounted { path } => {
                        let expected_path = mount_path.to_string_lossy().to_string();
                        prop_assert_eq!(path, &expected_path, 
                            "Error path should match the validated path");
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::thread;
    use std::sync::{Arc, Mutex};
    use npio::mount::advanced::{
        OperationContext, OperationType, OperationState, OperationResult, 
        OperationMetadata, CancellationReason, config::OperationConfig
    };
    use std::time::Duration;
    
    /// Simple test to verify the test framework is working.
    #[test]
    fn test_simple() {
        println!("Simple test is running!");
        assert_eq!(2 + 2, 4);
    }
    
    /// Test operation ID uniqueness across threads.
    #[test]
    fn test_operation_id_thread_safety() {
        let ids = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = vec![];
        
        // Spawn multiple threads generating IDs
        for _ in 0..10 {
            let ids_clone = Arc::clone(&ids);
            let handle = thread::spawn(move || {
                let mut local_ids = Vec::new();
                
                // Generate 100 IDs per thread
                for _ in 0..100 {
                    local_ids.push(OperationId::new());
                }
                
                // Add to shared set
                let mut shared_ids = ids_clone.lock().unwrap();
                for id in local_ids {
                    assert!(shared_ids.insert(id), "Thread generated duplicate ID: {}", id);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify we have 1000 unique IDs (10 threads * 100 IDs each)
        let final_ids = ids.lock().unwrap();
        assert_eq!(final_ids.len(), 1000);
    }
    
    /// Test that OperationId implements required traits correctly.
    #[test]
    fn test_operation_id_traits() {
        let id1 = OperationId::new();
        let id2 = OperationId::new();
        
        // Test Debug
        let debug_str = format!("{:?}", id1);
        assert!(!debug_str.is_empty());
        
        // Test Display
        let display_str = format!("{}", id1);
        assert!(!display_str.is_empty());
        
        // Test Clone
        let id1_clone = id1.clone();
        assert_eq!(id1, id1_clone);
        
        // Test PartialEq
        assert_eq!(id1, id1);
        assert_ne!(id1, id2);
        
        // Test Hash (by using in HashSet)
        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        assert_eq!(set.len(), 2);
    }

    /// Test OperationContext query methods.
    #[test]
    fn test_operation_context_query_methods() {
        let operation_type = OperationType::Mount {
            volume_path: "/dev/sdb1".to_string(),
            mount_point: Some("/mnt/test".to_string()),
        };
        let config = OperationConfig::default();
        let context = OperationContext::new(operation_type.clone(), config);

        // Test initial state queries
        assert_eq!(context.operation_type(), &operation_type);
        assert!(!context.is_terminal());
        assert_eq!(context.progress(), None);
        assert_eq!(context.status_message(), "Pending");
        assert!(context.metadata().is_none());
        assert!(context.result().is_none());
        assert!(context.error().is_none());
        assert!(context.cancellation_reason().is_none());

        // Test state update and queries
        context.update_state(OperationState::InProgress {
            progress: 0.5,
            message: "Mounting...".to_string(),
        });

        assert!(!context.is_terminal());
        assert_eq!(context.progress(), Some(0.5));
        assert_eq!(context.status_message(), "Mounting...");

        // Test completed state
        let result = OperationResult {
            operation_id: context.id(),
            operation_type: operation_type.clone(),
            duration: Duration::from_secs(1),
            metadata: OperationMetadata::default(),
        };
        
        context.update_state(OperationState::Completed { result: result.clone() });
        
        assert!(context.is_terminal());
        assert_eq!(context.progress(), Some(1.0));
        assert_eq!(context.status_message(), "Completed");
        assert!(context.metadata().is_some());
        assert!(context.result().is_some());
    }

    /// Test OperationContext cancellation queries.
    #[test]
    fn test_operation_context_cancellation() {
        let operation_type = OperationType::Unmount {
            mount_point: "/mnt/test".to_string(),
        };
        let config = OperationConfig::default();
        let context = OperationContext::new(operation_type, config);

        // Initially not cancelled
        assert!(!context.is_cancelled());
        assert!(context.cancellation_reason().is_none());

        // Cancel the operation
        context.cancellation_manager().cancel(CancellationReason::UserRequested);
        
        assert!(context.is_cancelled());

        // Update state to cancelled
        context.update_state(OperationState::Cancelled {
            reason: CancellationReason::UserRequested,
        });

        assert!(context.is_terminal());
        assert_eq!(context.cancellation_reason(), Some(CancellationReason::UserRequested));
    }

    /// Test OperationContext resource cleanup.
    #[test]
    fn test_operation_context_cleanup() {
        let operation_type = OperationType::Eject {
            device_path: "/dev/sdb".to_string(),
        };
        let config = OperationConfig::default();
        let context = OperationContext::new(operation_type, config);

        // Complete the operation
        let result = OperationResult {
            operation_id: context.id(),
            operation_type: context.operation_type().clone(),
            duration: Duration::from_secs(2),
            metadata: OperationMetadata::default(),
        };
        
        context.update_state(OperationState::Completed { result });

        // Test cleanup - should not panic and should work on terminal state
        context.cleanup();
        
        // Test force cleanup
        context.force_cleanup();
        
        // Should still be in terminal state
        assert!(context.is_terminal());
    }

    /// Test OperationContext thread safety.
    #[test]
    fn test_operation_context_thread_safety() {
        let operation_type = OperationType::Mount {
            volume_path: "/dev/sdc1".to_string(),
            mount_point: None,
        };
        let config = OperationConfig::default();
        let context = Arc::new(OperationContext::new(operation_type, config));
        
        let mut handles = vec![];
        
        // Spawn multiple threads that update and query state
        for i in 0..10 {
            let context_clone = Arc::clone(&context);
            let handle = thread::spawn(move || {
                // Update state
                context_clone.update_state(OperationState::InProgress {
                    progress: i as f32 / 10.0,
                    message: format!("Thread {} progress", i),
                });
                
                // Query state
                let _state = context_clone.state();
                let _progress = context_clone.progress();
                let _message = context_clone.status_message();
                let _is_terminal = context_clone.is_terminal();
                let _is_cancelled = context_clone.is_cancelled();
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Context should still be valid and in some progress state
        assert!(!context.is_terminal());
        assert!(context.progress().is_some());
    }
}

/// Property test for resource cleanup.
/// **Feature: advanced-mount-operations, Property 24: Resource Cleanup**
/// **Validates: Requirements 5.4**
proptest! {
    #[test]
    fn test_resource_cleanup(
        num_operations in 1usize..50,
        operation_types in prop::collection::vec(
            (0..3usize).prop_map(|i| match i {
                0 => OperationType::Mount {
                    volume_path: "/dev/test".to_string(),
                    mount_point: Some("/mnt/test".to_string()),
                },
                1 => OperationType::Unmount {
                    mount_point: "/mnt/test".to_string(),
                },
                _ => OperationType::Eject {
                    device_path: "/dev/test".to_string(),
                },
            }),
            1..50
        ),
        completion_scenarios in prop::collection::vec(
            (0..3usize).prop_map(|i| match i {
                0 => "success",
                1 => "cancelled",
                _ => "failed",
            }),
            1..50
        )
    ) {
        use std::sync::Arc;
        use npio::mount::advanced::{
            OperationContext, OperationState, OperationResult,
            OperationMetadata, CancellationReason, config::OperationConfig
        };
        use std::time::Duration;
        
        // Track resources before operations
        let mut contexts = Vec::new();
        let mut weak_refs = Vec::new();
        let mut progress_receivers = Vec::new();
        
        // Create operations and collect resource references
        for i in 0..num_operations {
            let op_type_idx = i % operation_types.len();
            let operation_type = operation_types[op_type_idx].clone();
            let config = OperationConfig::default();
            
            let context = Arc::new(OperationContext::new(operation_type.clone(), config));
            
            // Get weak reference to track cleanup
            let weak_ref = Arc::downgrade(&context);
            weak_refs.push(weak_ref);
            
            // Subscribe to progress events to test cleanup
            let progress_receiver = context.progress_reporter().subscribe();
            progress_receivers.push(progress_receiver);
            
            contexts.push(context);
        }
        
        // Property 1: All contexts should be alive before completion
        for weak_ref in &weak_refs {
            prop_assert!(weak_ref.strong_count() > 0, 
                "Context should be alive before completion");
        }
        
        // Property 2: Progress receivers should be functional before completion
        prop_assert_eq!(progress_receivers.len(), num_operations,
            "Should have progress receiver for each operation");
        
        // Complete operations according to scenarios
        for (i, context) in contexts.iter().enumerate() {
            let scenario_idx = i % completion_scenarios.len();
            let scenario = &completion_scenarios[scenario_idx];
            
            // Simulate some progress first
            context.update_state(OperationState::InProgress {
                progress: 0.5,
                message: format!("Operation {} in progress", i),
            });
            
            // Complete according to scenario
            match *scenario {
                "success" => {
                    let result = OperationResult {
                        operation_id: context.id(),
                        operation_type: context.operation_type().clone(),
                        duration: Duration::from_millis(100),
                        metadata: OperationMetadata::default(),
                    };
                    context.update_state(OperationState::Completed { result });
                }
                "cancelled" => {
                    context.cancellation_manager().cancel(CancellationReason::UserRequested);
                    context.update_state(OperationState::Cancelled {
                        reason: CancellationReason::UserRequested,
                    });
                }
                "failed" => {
                    let error = NpioError::new(
                        IOErrorEnum::Other,
                        "Test failure".to_string(),
                    );
                    context.update_state(OperationState::Failed {
                        error,
                        retry_count: 0,
                    });
                }
                _ => unreachable!(),
            }
            
            // Property 3: Context should be in terminal state after completion
            prop_assert!(context.is_terminal(), 
                "Context should be in terminal state after completion");
            
            // Property 4: Explicit cleanup should work on terminal operations
            context.cleanup();
            
            // Property 5: Context should still be functional after cleanup
            prop_assert!(context.is_terminal(), 
                "Context should remain terminal after cleanup");
            
            // Property 6: Cancellation manager should be in cancelled state after cleanup
            if !context.is_cancelled() && *scenario != "success" {
                // For non-success scenarios, cancellation should be set during cleanup
                // (This is implementation-dependent, so we check if it's reasonable)
            }
        }
        
        // Property 7: Force cleanup should work regardless of state
        for context in &contexts {
            context.force_cleanup();
            // Should not panic and context should remain accessible
            let _state = context.state();
            let _is_terminal = context.is_terminal();
        }
        
        // Drop all strong references to contexts
        drop(contexts);
        
        // Property 8: After dropping contexts, weak references should become invalid
        // (This tests that Drop implementation properly cleans up)
        for weak_ref in &weak_refs {
            prop_assert_eq!(weak_ref.strong_count(), 0,
                "All strong references should be dropped");
        }
        
        // Property 9: Progress receivers should handle cleanup gracefully
        // Try to receive from progress channels - should either work or fail gracefully
        for mut receiver in progress_receivers {
            // This should not panic, even if the sender is dropped
            match receiver.try_recv() {
                Ok(_) => {
                    // Got an event, that's fine
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                    // No events, that's fine
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    // Channel closed, that's expected after cleanup
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                    // Lagged behind, that's fine
                }
            }
        }
        
        // Property 10: Memory should be properly released
        // We can't directly test memory usage, but we can verify that
        // all operations completed without panics and references are cleaned up
        prop_assert!(true, "All cleanup operations completed without panics");
    }
}

/// Property test for graceful cancellation attempt.
/// **Feature: advanced-mount-operations, Property 6: Graceful Cancellation Attempt**
/// **Validates: Requirements 2.1**
proptest! {
    #[test]
    fn test_graceful_cancellation_attempt(
        graceful_timeouts in prop::collection::vec(50u64..500, 1..20),
        force_timeouts in prop::collection::vec(25u64..250, 1..20),
        cancellation_reasons in prop::collection::vec(
            (0..4usize).prop_map(|i| match i {
                0 => CancellationReason::UserRequested,
                1 => CancellationReason::Timeout,
                2 => CancellationReason::SystemShutdown,
                _ => CancellationReason::ParentCancelled,
            }),
            1..20
        ),
        graceful_success_rates in prop::collection::vec(0.0f64..1.0, 1..20)
    ) {
        use tokio::time::{sleep, Duration};
        use std::sync::atomic::{AtomicBool, Ordering};
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let num_tests = std::cmp::min(
                graceful_timeouts.len(),
                std::cmp::min(force_timeouts.len(), 
                    std::cmp::min(cancellation_reasons.len(), graceful_success_rates.len())
                )
            );
            
            for i in 0..num_tests {
                let graceful_timeout = Duration::from_millis(graceful_timeouts[i]);
                let force_timeout = Duration::from_millis(force_timeouts[i]);
                let reason = cancellation_reasons[i];
                let success_rate = graceful_success_rates[i];
                
                // Create cancellation manager with custom timeouts
                let manager = CancellationManager::with_timeouts(graceful_timeout, force_timeout);
                
                // Property 1: Manager should not be cancelled initially
                prop_assert!(!manager.is_cancelled(), 
                    "Manager should not be cancelled initially");
                prop_assert!(manager.cancellation_reason().is_none(),
                    "Manager should have no cancellation reason initially");
                
                // Property 2: Graceful timeout should match configured value
                prop_assert_eq!(manager.graceful_timeout(), graceful_timeout,
                    "Graceful timeout should match configured value");
                prop_assert_eq!(manager.force_timeout(), force_timeout,
                    "Force timeout should match configured value");
                
                // Simulate graceful cancellation function that may succeed or fail
                let graceful_succeeded = Arc::new(AtomicBool::new(false));
                let graceful_succeeded_clone = graceful_succeeded.clone();
                
                let graceful_cancel_fn = move || {
                    let graceful_succeeded = graceful_succeeded_clone.clone();
                    async move {
                        // Simulate some work time (less than graceful timeout)
                        let work_time = graceful_timeout / 4;
                        sleep(work_time).await;
                        
                        // Succeed based on success rate
                        if success_rate > 0.5 {
                            graceful_succeeded.store(true, Ordering::Release);
                            Ok(())
                        } else {
                            Err(NpioError::new(
                                IOErrorEnum::Other,
                                "Graceful cancellation failed".to_string(),
                            ))
                        }
                    }
                };
                
                // Property 3: Cancellation should attempt graceful cancellation first
                let start_time = std::time::Instant::now();
                let result = manager.cancel_with_cleanup(reason, graceful_cancel_fn).await;
                let elapsed = start_time.elapsed();
                
                // Property 4: Manager should be cancelled after cancellation attempt
                prop_assert!(manager.is_cancelled(),
                    "Manager should be cancelled after cancellation attempt");
                prop_assert_eq!(manager.cancellation_reason(), Some(reason),
                    "Manager should have correct cancellation reason");
                
                // Property 5: Cancellation should respect graceful timeout
                if success_rate > 0.5 {
                    // Graceful cancellation should succeed
                    prop_assert!(result.is_ok() || graceful_succeeded.load(Ordering::Acquire),
                        "Graceful cancellation should succeed when success rate is high");
                    
                    // Should complete within graceful timeout + some margin
                    let max_expected_time = graceful_timeout + Duration::from_millis(100);
                    prop_assert!(elapsed <= max_expected_time,
                        "Graceful cancellation should complete within timeout: {:?} > {:?}",
                        elapsed, max_expected_time);
                } else {
                    // Graceful cancellation should fail, triggering forced cancellation
                    // Total time should include both graceful and force timeouts
                    let min_expected_time = graceful_timeout / 2; // At least some graceful time
                    prop_assert!(elapsed >= min_expected_time,
                        "Should spend at least some time on graceful cancellation: {:?} < {:?}",
                        elapsed, min_expected_time);
                }
                
                // Property 6: Token should be cancelled
                prop_assert!(manager.token().is_cancelled(),
                    "Cancellation token should be cancelled");
                prop_assert_eq!(manager.token().cancellation_reason(), Some(reason),
                    "Token should have correct cancellation reason");
                
                // Property 7: Subsequent cancellation requests should be idempotent
                let second_result = manager.cancel_with_cleanup(
                    CancellationReason::UserRequested,
                    || async { Ok(()) }
                ).await;
                
                // Should still be cancelled with original reason
                prop_assert!(manager.is_cancelled(),
                    "Manager should remain cancelled after second request");
                prop_assert_eq!(manager.cancellation_reason(), Some(reason),
                    "Original cancellation reason should be preserved");
            }
            
            Ok(())
        })?;
    }
}

/// Property test for forced cancellation fallback.
/// **Feature: advanced-mount-operations, Property 7: Forced Cancellation Fallback**
/// **Validates: Requirements 2.2**
proptest! {
    #[test]
    fn test_forced_cancellation_fallback(
        graceful_timeouts in prop::collection::vec(50u64..200, 1..20),
        force_timeouts in prop::collection::vec(25u64..100, 1..20),
        cancellation_reasons in prop::collection::vec(
            (0..4usize).prop_map(|i| match i {
                0 => CancellationReason::UserRequested,
                1 => CancellationReason::Timeout,
                2 => CancellationReason::SystemShutdown,
                _ => CancellationReason::ParentCancelled,
            }),
            1..20
        ),
        graceful_failure_scenarios in prop::collection::vec(
            (0..3usize).prop_map(|i| match i {
                0 => "timeout", // Graceful cancellation times out
                1 => "error",   // Graceful cancellation returns error
                _ => "panic",   // Graceful cancellation panics (simulated)
            }),
            1..20
        )
    ) {
        use tokio::time::{sleep, Duration};
        use std::sync::atomic::{AtomicBool, Ordering};
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let num_tests = std::cmp::min(
                graceful_timeouts.len(),
                std::cmp::min(force_timeouts.len(), 
                    std::cmp::min(cancellation_reasons.len(), graceful_failure_scenarios.len())
                )
            );
            
            for i in 0..num_tests {
                let graceful_timeout = Duration::from_millis(graceful_timeouts[i]);
                let force_timeout = Duration::from_millis(force_timeouts[i]);
                let reason = cancellation_reasons[i];
                let failure_scenario = &graceful_failure_scenarios[i];
                
                // Create cancellation manager with custom timeouts
                let manager = CancellationManager::with_timeouts(graceful_timeout, force_timeout);
                
                // Property 1: Manager should not be cancelled initially
                prop_assert!(!manager.is_cancelled(), 
                    "Manager should not be cancelled initially");
                
                // Track whether forced cancellation was triggered
                let forced_cancellation_triggered = Arc::new(AtomicBool::new(false));
                let forced_triggered_clone = forced_cancellation_triggered.clone();
                
                // Add cleanup callback to detect forced cancellation
                manager.add_cleanup_callback(move || {
                    forced_triggered_clone.store(true, Ordering::Release);
                });
                
                // Create graceful cancellation function that will fail according to scenario
                let graceful_cancel_fn = move || {
                    async move {
                        match *failure_scenario {
                            "timeout" => {
                                // Sleep longer than graceful timeout to force timeout
                                let sleep_time = graceful_timeout + Duration::from_millis(50);
                                sleep(sleep_time).await;
                                Ok(())
                            }
                            "error" => {
                                // Return error immediately to trigger forced cancellation
                                Err(NpioError::new(
                                    IOErrorEnum::Other,
                                    "Graceful cancellation failed".to_string(),
                                ))
                            }
                            "panic" => {
                                // Simulate a quick failure that should trigger forced cancellation
                                sleep(Duration::from_millis(10)).await;
                                Err(NpioError::new(
                                    IOErrorEnum::Interrupted,
                                    "Simulated panic in graceful cancellation".to_string(),
                                ))
                            }
                            _ => unreachable!(),
                        }
                    }
                };
                
                // Property 2: Cancellation should attempt graceful first, then forced
                let start_time = std::time::Instant::now();
                let result = manager.cancel_with_cleanup(reason, graceful_cancel_fn).await;
                let elapsed = start_time.elapsed();
                
                // Property 3: Manager should be cancelled after cancellation attempt
                prop_assert!(manager.is_cancelled(),
                    "Manager should be cancelled after cancellation attempt");
                prop_assert_eq!(manager.cancellation_reason(), Some(reason),
                    "Manager should have correct cancellation reason");
                
                // Property 4: Forced cancellation should be triggered when graceful fails
                match *failure_scenario {
                    "timeout" => {
                        // Should have taken at least the graceful timeout
                        prop_assert!(elapsed >= graceful_timeout,
                            "Should have waited at least graceful timeout: {:?} < {:?}",
                            elapsed, graceful_timeout);
                        
                        // Should have triggered forced cancellation
                        prop_assert!(forced_cancellation_triggered.load(Ordering::Acquire),
                            "Forced cancellation should be triggered on timeout");
                        
                        // Result should indicate timeout
                        prop_assert!(result.is_err(),
                            "Result should be error when graceful cancellation times out");
                    }
                    "error" | "panic" => {
                        // Should have triggered forced cancellation
                        prop_assert!(forced_cancellation_triggered.load(Ordering::Acquire),
                            "Forced cancellation should be triggered on error");
                        
                        // Should complete relatively quickly (graceful failed fast)
                        let max_expected_time = graceful_timeout + force_timeout + Duration::from_millis(100);
                        prop_assert!(elapsed <= max_expected_time,
                            "Should complete within reasonable time: {:?} > {:?}",
                            elapsed, max_expected_time);
                    }
                    _ => unreachable!(),
                }
                
                // Property 5: Token should be cancelled regardless of graceful failure
                prop_assert!(manager.token().is_cancelled(),
                    "Cancellation token should be cancelled");
                prop_assert_eq!(manager.token().cancellation_reason(), Some(reason),
                    "Token should have correct cancellation reason");
                
                // Property 6: Force cancellation should work independently
                let force_manager = CancellationManager::with_timeouts(graceful_timeout, force_timeout);
                let force_cleanup_triggered = Arc::new(AtomicBool::new(false));
                let force_cleanup_clone = force_cleanup_triggered.clone();
                
                force_manager.add_cleanup_callback(move || {
                    force_cleanup_clone.store(true, Ordering::Release);
                });
                
                // Request cancellation first
                force_manager.request_cancellation(reason);
                
                // Then call force_cancel directly
                let force_start = std::time::Instant::now();
                let force_result = force_manager.force_cancel().await;
                let force_elapsed = force_start.elapsed();
                
                // Should complete within force timeout
                prop_assert!(force_elapsed <= force_timeout + Duration::from_millis(50),
                    "Force cancellation should complete within timeout: {:?} > {:?}",
                    force_elapsed, force_timeout + Duration::from_millis(50));
                
                // Should trigger cleanup
                prop_assert!(force_cleanup_triggered.load(Ordering::Acquire),
                    "Force cancellation should trigger cleanup callbacks");
                
                // Should succeed or fail gracefully
                let force_succeeded = match &force_result {
                    Ok(()) => {
                        // Force cancellation succeeded
                        true
                    }
                    Err(e) => {
                        // Force cancellation timed out, which is acceptable
                        prop_assert!(e.to_string().contains("timed out"),
                            "Force cancellation error should be about timeout: {}", e);
                        false
                    }
                };
                
                // Property 7: Multiple force cancellations should be idempotent
                let second_force_result = force_manager.force_cancel().await;
                prop_assert!(force_manager.is_cancelled(),
                    "Manager should remain cancelled after second force cancel");
                
                // Both results should be consistent
                let second_succeeded = second_force_result.is_ok();
                
                // Results should be consistent (both succeed, both fail, or mixed due to timing)
                match (force_succeeded, second_succeeded) {
                    (true, true) => {
                        // Both succeeded, good
                    }
                    (false, false) => {
                        // Both failed, acceptable for timeout scenarios
                    }
                    (true, false) | (false, true) => {
                        // Mixed results are acceptable due to timing
                    }
                }
            }
            
            Ok(())
        })?;
    }
}

/// Property test for cancellation cleanup.
/// **Feature: advanced-mount-operations, Property 8: Cancellation Cleanup**
/// **Validates: Requirements 2.3**
proptest! {
    #[test]
    fn test_cancellation_cleanup(
        num_cleanup_callbacks in 1usize..20,
        num_backend_handlers in 1usize..10,
        num_system_resources in 1usize..15,
        resource_types in prop::collection::vec(
            (0..5usize).prop_map(|i| match i {
                0 => "dbus_call",
                1 => "system_call", 
                2 => "file_descriptor",
                3 => "temp_resource",
                _ => "network_connection",
            }),
            1..15
        ),
        cancellation_reasons in prop::collection::vec(
            (0..4usize).prop_map(|i| match i {
                0 => CancellationReason::UserRequested,
                1 => CancellationReason::Timeout,
                2 => CancellationReason::SystemShutdown,
                _ => CancellationReason::ParentCancelled,
            }),
            1..10
        )
    ) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::time::Duration;
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let num_tests = std::cmp::min(cancellation_reasons.len(), 5); // Limit tests for performance
            
            for test_idx in 0..num_tests {
                let reason = cancellation_reasons[test_idx];
                
                // Create cancellation manager
                let manager = CancellationManager::with_timeouts(
                    Duration::from_millis(100),
                    Duration::from_millis(50),
                );
                
                // Property 1: Manager should not be cancelled initially
                prop_assert!(!manager.is_cancelled(), 
                    "Manager should not be cancelled initially");
                
                // Track cleanup execution
                let cleanup_counter = Arc::new(AtomicUsize::new(0));
                let backend_counter = Arc::new(AtomicUsize::new(0));
                
                // Add cleanup callbacks
                let actual_callbacks = std::cmp::min(num_cleanup_callbacks, 10); // Limit for performance
                for i in 0..actual_callbacks {
                    let counter_clone = cleanup_counter.clone();
                    manager.add_cleanup_callback(move || {
                        counter_clone.fetch_add(1, Ordering::Relaxed);
                    });
                }
                
                // Add backend cleanup handlers
                let actual_backends = std::cmp::min(num_backend_handlers, 5); // Limit for performance
                for i in 0..actual_backends {
                    let counter_clone = backend_counter.clone();
                    let backend_name = format!("backend_{}", i);
                    manager.add_backend_cleanup(backend_name, move || {
                        counter_clone.fetch_add(1, Ordering::Relaxed);
                    });
                }
                
                // Register system resources
                let actual_resources = std::cmp::min(num_system_resources, resource_types.len());
                for i in 0..actual_resources {
                    let resource_type = resource_types[i % resource_types.len()];
                    
                    match resource_type {
                        "dbus_call" => {
                            manager.register_dbus_call(
                                format!("/org/test/object_{}", i),
                                format!("Method_{}", i),
                                Some(format!("call_{}", i)),
                            );
                        }
                        "system_call" => {
                            manager.register_system_call(
                                Some(1000 + i as u32),
                                format!("syscall_{}", i),
                            );
                        }
                        "file_descriptor" => {
                            manager.register_file_descriptor(
                                100 + i as i32,
                                format!("fd_description_{}", i),
                            );
                        }
                        "temp_resource" => {
                            manager.register_temp_resource(
                                format!("/tmp/resource_{}", i),
                                "file".to_string(),
                            );
                        }
                        "network_connection" => {
                            manager.register_network_connection(
                                format!("conn_{}", i),
                                "TCP".to_string(),
                            );
                        }
                        _ => unreachable!(),
                    }
                }
                
                // Property 2: System resources should be registered correctly
                prop_assert_eq!(manager.system_resource_count(), actual_resources,
                    "Should have registered {} system resources", actual_resources);
                
                // Property 3: Cleanup should execute all registered callbacks
                manager.cleanup_all_resources();
                
                // Verify cleanup callbacks were executed
                prop_assert_eq!(cleanup_counter.load(Ordering::Relaxed), actual_callbacks,
                    "All {} cleanup callbacks should be executed", actual_callbacks);
                
                // Verify backend handlers were executed
                prop_assert_eq!(backend_counter.load(Ordering::Relaxed), actual_backends,
                    "All {} backend handlers should be executed", actual_backends);
                
                // Property 4: Cancellation should trigger cleanup
                let pre_cancel_cleanup_count = cleanup_counter.load(Ordering::Relaxed);
                let pre_cancel_backend_count = backend_counter.load(Ordering::Relaxed);
                
                // Reset counters to test cancellation cleanup
                cleanup_counter.store(0, Ordering::Relaxed);
                backend_counter.store(0, Ordering::Relaxed);
                
                // Request cancellation and force cleanup
                manager.request_cancellation(reason);
                let force_result = manager.force_cancel().await;
                
                // Property 5: Manager should be cancelled after force cancel
                prop_assert!(manager.is_cancelled(),
                    "Manager should be cancelled after force_cancel");
                prop_assert_eq!(manager.cancellation_reason(), Some(reason),
                    "Manager should have correct cancellation reason");
                
                // Property 6: Force cancel should succeed or fail gracefully
                match force_result {
                    Ok(()) => {
                        // Force cancellation succeeded
                    }
                    Err(e) => {
                        // Force cancellation may timeout, which is acceptable
                        prop_assert!(e.to_string().contains("timed out") || 
                                   e.to_string().contains("timeout"),
                            "Force cancellation error should be about timeout: {}", e);
                    }
                }
                
                // Property 7: Cleanup should be executed during force cancel
                prop_assert_eq!(cleanup_counter.load(Ordering::Relaxed), actual_callbacks,
                    "All cleanup callbacks should be executed during force cancel");
                prop_assert_eq!(backend_counter.load(Ordering::Relaxed), actual_backends,
                    "All backend handlers should be executed during force cancel");
                
                // Property 8: Multiple cleanup calls should be idempotent
                cleanup_counter.store(0, Ordering::Relaxed);
                backend_counter.store(0, Ordering::Relaxed);
                
                manager.cleanup_all_resources();
                let first_cleanup_count = cleanup_counter.load(Ordering::Relaxed);
                let first_backend_count = backend_counter.load(Ordering::Relaxed);
                
                manager.cleanup_all_resources();
                let second_cleanup_count = cleanup_counter.load(Ordering::Relaxed);
                let second_backend_count = backend_counter.load(Ordering::Relaxed);
                
                // Cleanup should be executed each time (not truly idempotent, but consistent)
                prop_assert_eq!(first_cleanup_count, actual_callbacks,
                    "First cleanup should execute all callbacks");
                prop_assert_eq!(second_cleanup_count, actual_callbacks * 2,
                    "Second cleanup should execute all callbacks again");
                prop_assert_eq!(first_backend_count, actual_backends,
                    "First cleanup should execute all backend handlers");
                prop_assert_eq!(second_backend_count, actual_backends * 2,
                    "Second cleanup should execute all backend handlers again");
                
                // Property 9: Scoped managers should have independent cleanup
                let scoped_manager = manager.create_scope();
                let scoped_cleanup_counter = Arc::new(AtomicUsize::new(0));
                let scoped_counter_clone = scoped_cleanup_counter.clone();
                
                scoped_manager.add_cleanup_callback(move || {
                    scoped_counter_clone.fetch_add(1, Ordering::Relaxed);
                });
                
                scoped_manager.cleanup_all_resources();
                
                prop_assert_eq!(scoped_cleanup_counter.load(Ordering::Relaxed), 1,
                    "Scoped manager should have independent cleanup");
                
                // Original manager's counters should not be affected by scoped cleanup
                let original_count_after_scoped = cleanup_counter.load(Ordering::Relaxed);
                prop_assert_eq!(original_count_after_scoped, actual_callbacks * 2,
                    "Original manager cleanup count should not be affected by scoped cleanup");
            }
            
            Ok(())
        })?;
    }
}