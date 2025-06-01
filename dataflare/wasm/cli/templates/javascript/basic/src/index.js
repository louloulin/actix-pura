/**
 * {{plugin_name}} - DataFlare {{plugin_type}} Plugin
 * 
 * {{description}}
 * 
 * @author {{author}}
 * @version 0.1.0
 */

{{#if (eq plugin_type "filter")}}
/**
 * Filter function - determines whether a record should be kept or filtered out
 * 
 * @param {Object} record - The input data record
 * @param {Uint8Array} record.value - The record data as bytes
 * @param {Object} record.metadata - Record metadata
 * @returns {boolean} - true to keep the record, false to filter it out
 */
export function process(record) {
    try {
        // Convert bytes to string for processing
        const data = new TextDecoder().decode(record.value);
        
        // TODO: Implement your filter logic here
        // Example: filter records containing "error"
        return data.toLowerCase().includes("error");
        
    } catch (error) {
        console.error("Filter processing error:", error);
        return false;
    }
}
{{/if}}

{{#if (eq plugin_type "map")}}
/**
 * Map function - transforms input data to output data
 * 
 * @param {Object} record - The input data record
 * @param {Uint8Array} record.value - The record data as bytes
 * @param {Object} record.metadata - Record metadata
 * @returns {Uint8Array} - The transformed data as bytes
 */
export function process(record) {
    try {
        // Convert bytes to string for processing
        const data = new TextDecoder().decode(record.value);
        
        // TODO: Implement your transformation logic here
        // Example: convert to uppercase
        const transformed = data.toUpperCase();
        
        // Convert back to bytes
        return new TextEncoder().encode(transformed);
        
    } catch (error) {
        console.error("Map processing error:", error);
        // Return original data on error
        return record.value;
    }
}
{{/if}}

{{#if (eq plugin_type "aggregate")}}
// Global state for aggregation
let aggregateState = {
    count: 0,
    sum: 0,
    values: []
};

/**
 * Aggregate function - accumulates data across multiple records
 * 
 * @param {Object} record - The input data record
 * @param {Uint8Array} record.value - The record data as bytes
 * @param {Object} record.metadata - Record metadata
 * @returns {Uint8Array} - The aggregated result as bytes
 */
export function process(record) {
    try {
        // Convert bytes to string for processing
        const data = new TextDecoder().decode(record.value);
        
        // TODO: Implement your aggregation logic here
        // Example: count records and sum numeric values
        aggregateState.count++;
        
        // Try to parse as number
        const numValue = parseFloat(data);
        if (!isNaN(numValue)) {
            aggregateState.sum += numValue;
            aggregateState.values.push(numValue);
        }
        
        // Return current aggregate state
        const result = {
            count: aggregateState.count,
            sum: aggregateState.sum,
            average: aggregateState.values.length > 0 ? 
                aggregateState.sum / aggregateState.values.length : 0,
            latest_value: data
        };
        
        return new TextEncoder().encode(JSON.stringify(result));
        
    } catch (error) {
        console.error("Aggregate processing error:", error);
        return new TextEncoder().encode(JSON.stringify({ error: error.message }));
    }
}
{{/if}}

{{#if (eq plugin_type "processor")}}
/**
 * Process function - general data processing
 * 
 * @param {Object} record - The input data record
 * @param {Uint8Array} record.value - The record data as bytes
 * @param {Object} record.metadata - Record metadata
 * @returns {Uint8Array} - The processed data as bytes
 */
export function process(record) {
    try {
        // Convert bytes to string for processing
        const data = new TextDecoder().decode(record.value);
        
        // TODO: Implement your processing logic here
        // Example: add timestamp and processing info
        let processedData;
        
        try {
            // Try to parse as JSON
            const jsonData = JSON.parse(data);
            processedData = {
                ...jsonData,
                processed_at: new Date().toISOString(),
                processed_by: "{{plugin_name}}",
                processor_version: "0.1.0"
            };
        } catch {
            // If not JSON, treat as plain text
            processedData = {
                original_data: data,
                processed_at: new Date().toISOString(),
                processed_by: "{{plugin_name}}",
                processor_version: "0.1.0"
            };
        }
        
        return new TextEncoder().encode(JSON.stringify(processedData));
        
    } catch (error) {
        console.error("Processing error:", error);
        return record.value; // Return original data on error
    }
}
{{/if}}

/**
 * Plugin information function
 * 
 * @returns {Array} - [name, version] tuple
 */
export function info() {
    return ["{{plugin_name}}", "0.1.0"];
}

/**
 * Plugin capabilities function
 * 
 * @returns {Object} - Plugin capabilities and metadata
 */
export function capabilities() {
    return {
        name: "{{plugin_name}}",
        version: "0.1.0",
        type: "{{plugin_type}}",
        language: "javascript",
        description: "{{description}}",
        author: "{{author}}",
        supports_batch: false,
        supports_streaming: true,
        memory_requirements: "low",
        cpu_requirements: "low"
    };
}

/**
 * Plugin initialization function (optional)
 * Called once when the plugin is loaded
 * 
 * @param {Object} config - Plugin configuration
 */
export function initialize(config) {
    console.log(`Initializing {{plugin_name}} with config:`, config);
    
    // TODO: Add any initialization logic here
    // Example: validate configuration, set up connections, etc.
}

/**
 * Plugin cleanup function (optional)
 * Called when the plugin is unloaded
 */
export function cleanup() {
    console.log(`Cleaning up {{plugin_name}}`);
    
    // TODO: Add any cleanup logic here
    // Example: close connections, save state, etc.
    
    // Reset aggregate state if this is an aggregate plugin
    {{#if (eq plugin_type "aggregate")}}
    aggregateState = {
        count: 0,
        sum: 0,
        values: []
    };
    {{/if}}
}
