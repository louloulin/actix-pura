use dataflare_core::{
    message::DataRecord,
    processor::Processor,
};
use dataflare_processor::DTLProcessor;
use serde_json::{json, Value};
use tokio;

#[tokio::test]
async fn test_dtl_field_assignment() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with simple field assignment
    let config = json!({
        "source": ".name = \"John Doe\"\n.age = 30\n.active = true"
    });
    
    processor.configure(&config).expect("Failed to configure DTL processor");
    processor.initialize().await.expect("Failed to initialize DTL processor");
    
    // Create test record
    let input_data = json!({
        "id": 1,
        "email": "john@example.com"
    });
    let record = DataRecord::new(input_data);
    
    // Process the record
    let result = processor.process_record(&record).await.expect("Failed to process record");
    
    // Verify the result
    assert_eq!(result.data["id"], 1);
    assert_eq!(result.data["email"], "john@example.com");
    assert_eq!(result.data["name"], "John Doe");
    assert_eq!(result.data["age"], 30);
    assert_eq!(result.data["active"], true);
}

#[tokio::test]
async fn test_dtl_field_deletion() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with field deletion
    let config = json!({
        "source": "del(.password)\ndel(.secret)"
    });
    
    processor.configure(&config).expect("Failed to configure DTL processor");
    processor.initialize().await.expect("Failed to initialize DTL processor");
    
    // Create test record
    let input_data = json!({
        "id": 1,
        "username": "john",
        "password": "secret123",
        "secret": "top_secret",
        "email": "john@example.com"
    });
    let record = DataRecord::new(input_data);
    
    // Process the record
    let result = processor.process_record(&record).await.expect("Failed to process record");
    
    // Verify the result
    assert_eq!(result.data["id"], 1);
    assert_eq!(result.data["username"], "john");
    assert_eq!(result.data["email"], "john@example.com");
    assert!(result.data.get("password").is_none());
    assert!(result.data.get("secret").is_none());
}

#[tokio::test]
async fn test_dtl_field_reference() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with field reference
    let config = json!({
        "source": ".full_name = .first_name\n.backup_email = .email"
    });
    
    processor.configure(&config).expect("Failed to configure DTL processor");
    processor.initialize().await.expect("Failed to initialize DTL processor");
    
    // Create test record
    let input_data = json!({
        "first_name": "John",
        "last_name": "Doe",
        "email": "john@example.com"
    });
    let record = DataRecord::new(input_data);
    
    // Process the record
    let result = processor.process_record(&record).await.expect("Failed to process record");
    
    // Verify the result
    assert_eq!(result.data["first_name"], "John");
    assert_eq!(result.data["last_name"], "Doe");
    assert_eq!(result.data["email"], "john@example.com");
    assert_eq!(result.data["full_name"], "John");
    assert_eq!(result.data["backup_email"], "john@example.com");
}

#[tokio::test]
async fn test_dtl_function_calls() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with function calls
    let config = json!({
        "source": ".timestamp = now()\n.name_length = length(.name)\n.greeting = concat(\"Hello, \", .name)"
    });
    
    processor.configure(&config).expect("Failed to configure DTL processor");
    processor.initialize().await.expect("Failed to initialize DTL processor");
    
    // Create test record
    let input_data = json!({
        "name": "John"
    });
    let record = DataRecord::new(input_data);
    
    // Process the record
    let result = processor.process_record(&record).await.expect("Failed to process record");
    
    // Verify the result
    assert_eq!(result.data["name"], "John");
    assert!(result.data["timestamp"].is_number());
    assert_eq!(result.data["name_length"], 4);
    assert_eq!(result.data["greeting"], "Hello, John");
}

#[tokio::test]
async fn test_dtl_complex_transformation() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with complex transformation
    let config = json!({
        "source": r#"
.user_id = .id
.display_name = concat(.first_name, " ", .last_name)
.email_domain = .email
.is_admin = false
del(.password)
del(.internal_id)
"#
    });
    
    processor.configure(&config).expect("Failed to configure DTL processor");
    processor.initialize().await.expect("Failed to initialize DTL processor");
    
    // Create test record
    let input_data = json!({
        "id": 123,
        "first_name": "Jane",
        "last_name": "Smith",
        "email": "jane.smith@company.com",
        "password": "secret",
        "internal_id": "INT_456",
        "department": "Engineering"
    });
    let record = DataRecord::new(input_data);
    
    // Process the record
    let result = processor.process_record(&record).await.expect("Failed to process record");
    
    // Verify the result
    assert_eq!(result.data["id"], 123);
    assert_eq!(result.data["user_id"], 123);
    assert_eq!(result.data["first_name"], "Jane");
    assert_eq!(result.data["last_name"], "Smith");
    assert_eq!(result.data["display_name"], "Jane Smith");
    assert_eq!(result.data["email"], "jane.smith@company.com");
    assert_eq!(result.data["email_domain"], "jane.smith@company.com");
    assert_eq!(result.data["is_admin"], false);
    assert_eq!(result.data["department"], "Engineering");
    assert!(result.data.get("password").is_none());
    assert!(result.data.get("internal_id").is_none());
}

#[tokio::test]
async fn test_dtl_error_handling() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with invalid syntax
    let config = json!({
        "source": "invalid syntax here"
    });
    
    // Should fail to configure
    assert!(processor.configure(&config).is_err());
}

#[tokio::test]
async fn test_dtl_empty_source() {
    let mut processor = DTLProcessor::new();
    
    // Configure DTL processor with empty source
    let config = json!({
        "source": ""
    });
    
    // Should fail to configure
    assert!(processor.configure(&config).is_err());
}
