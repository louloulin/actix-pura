// {{plugin_name}} - DataFlare {{plugin_type}} Plugin
//
// {{description}}
//
// Author: {{author}}
// Version: 0.1.0

package main

import (
	"encoding/json"
	"fmt"
	"log"
	"strings"
	"time"
)

{{#if (eq plugin_type "aggregate")}}
// Global state for aggregation
var aggregateState = struct {
	Count  int     `json:"count"`
	Sum    float64 `json:"sum"`
	Values []float64 `json:"values"`
}{
	Count:  0,
	Sum:    0,
	Values: make([]float64, 0),
}
{{/if}}

// DataRecord represents an input data record
type DataRecord struct {
	Value    []byte            `json:"value"`
	Metadata map[string]string `json:"metadata"`
}

// PluginInfo represents plugin information
type PluginInfo struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// PluginCapabilities represents plugin capabilities
type PluginCapabilities struct {
	Name               string `json:"name"`
	Version            string `json:"version"`
	Type               string `json:"type"`
	Language           string `json:"language"`
	Description        string `json:"description"`
	Author             string `json:"author"`
	SupportsBatch      bool   `json:"supports_batch"`
	SupportsStreaming  bool   `json:"supports_streaming"`
	MemoryRequirements string `json:"memory_requirements"`
	CPURequirements    string `json:"cpu_requirements"`
}

{{#if (eq plugin_type "filter")}}
// Process filters input data and returns true to keep, false to filter out
func Process(data []byte) (bool, error) {
	// Convert bytes to string for processing
	dataStr := string(data)
	
	// TODO: Implement your filter logic here
	// Example: filter records containing "error"
	return strings.Contains(strings.ToLower(dataStr), "error"), nil
}
{{/if}}

{{#if (eq plugin_type "map")}}
// Process transforms input data to output data
func Process(data []byte) ([]byte, error) {
	// Convert bytes to string for processing
	dataStr := string(data)
	
	// TODO: Implement your transformation logic here
	// Example: convert to uppercase
	transformed := strings.ToUpper(dataStr)
	
	return []byte(transformed), nil
}
{{/if}}

{{#if (eq plugin_type "aggregate")}}
// Process accumulates data across multiple records
func Process(data []byte) ([]byte, error) {
	// Convert bytes to string for processing
	dataStr := string(data)
	
	// TODO: Implement your aggregation logic here
	// Example: count records and sum numeric values
	aggregateState.Count++
	
	// Try to parse as number
	var numValue float64
	if err := json.Unmarshal(data, &numValue); err == nil {
		aggregateState.Sum += numValue
		aggregateState.Values = append(aggregateState.Values, numValue)
	}
	
	// Calculate average
	var average float64
	if len(aggregateState.Values) > 0 {
		average = aggregateState.Sum / float64(len(aggregateState.Values))
	}
	
	// Return current aggregate state
	result := map[string]interface{}{
		"count":        aggregateState.Count,
		"sum":          aggregateState.Sum,
		"average":      average,
		"latest_value": dataStr,
	}
	
	return json.Marshal(result)
}
{{/if}}

{{#if (eq plugin_type "processor")}}
// Process performs general data processing
func Process(data []byte) ([]byte, error) {
	// Convert bytes to string for processing
	dataStr := string(data)
	
	// TODO: Implement your processing logic here
	// Example: add timestamp and processing info
	var processedData map[string]interface{}
	
	// Try to parse as JSON
	var jsonData map[string]interface{}
	if err := json.Unmarshal(data, &jsonData); err == nil {
		// If JSON, add fields to existing object
		processedData = jsonData
		processedData["processed_at"] = time.Now().Format(time.RFC3339)
		processedData["processed_by"] = "{{plugin_name}}"
		processedData["processor_version"] = "0.1.0"
	} else {
		// If not JSON, create new object
		processedData = map[string]interface{}{
			"original_data":      dataStr,
			"processed_at":       time.Now().Format(time.RFC3339),
			"processed_by":       "{{plugin_name}}",
			"processor_version":  "0.1.0",
		}
	}
	
	return json.Marshal(processedData)
}
{{/if}}

// Info returns plugin name and version
func Info() (string, string) {
	return "{{plugin_name}}", "0.1.0"
}

// Capabilities returns plugin capabilities and metadata
func Capabilities() PluginCapabilities {
	return PluginCapabilities{
		Name:               "{{plugin_name}}",
		Version:            "0.1.0",
		Type:               "{{plugin_type}}",
		Language:           "go",
		Description:        "{{description}}",
		Author:             "{{author}}",
		SupportsBatch:      false,
		SupportsStreaming:  true,
		MemoryRequirements: "low",
		CPURequirements:    "low",
	}
}

// Initialize is called once when the plugin is loaded (optional)
func Initialize(config map[string]interface{}) error {
	log.Printf("Initializing {{plugin_name}} with config: %+v", config)
	
	// TODO: Add any initialization logic here
	// Example: validate configuration, set up connections, etc.
	
	return nil
}

// Cleanup is called when the plugin is unloaded (optional)
func Cleanup() error {
	log.Println("Cleaning up {{plugin_name}}")
	
	// TODO: Add any cleanup logic here
	// Example: close connections, save state, etc.
	
	{{#if (eq plugin_type "aggregate")}}
	// Reset aggregate state
	aggregateState.Count = 0
	aggregateState.Sum = 0
	aggregateState.Values = make([]float64, 0)
	{{/if}}
	
	return nil
}

// WASM export functions
//export process
func process() {
	// This will be implemented by the WASM runtime
}

//export info
func info() {
	// This will be implemented by the WASM runtime
}

//export capabilities
func capabilities() {
	// This will be implemented by the WASM runtime
}

//export initialize
func initialize() {
	// This will be implemented by the WASM runtime
}

//export cleanup
func cleanup() {
	// This will be implemented by the WASM runtime
}

func main() {
	// Main function for testing
	fmt.Printf("{{plugin_name}} v0.1.0 - DataFlare {{plugin_type}} Plugin\n")
	fmt.Printf("{{description}}\n")
	
	// Test the plugin functions
	name, version := Info()
	fmt.Printf("Plugin: %s v%s\n", name, version)
	
	caps := Capabilities()
	fmt.Printf("Type: %s, Language: %s\n", caps.Type, caps.Language)
	
	// Test processing with sample data
	testData := []byte(`{"test": "data", "value": 42}`)
	
	{{#if (eq plugin_type "filter")}}
	result, err := Process(testData)
	if err != nil {
		log.Printf("Error: %v", err)
	} else {
		fmt.Printf("Filter result: %t\n", result)
	}
	{{else}}
	result, err := Process(testData)
	if err != nil {
		log.Printf("Error: %v", err)
	} else {
		fmt.Printf("Process result: %s\n", string(result))
	}
	{{/if}}
}
