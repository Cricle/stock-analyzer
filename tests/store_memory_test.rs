use chrono::Utc;
use sa::store::{
    AnalysisStore, CacheStore, CheckpointStore, GuidanceStore, InMemoryAnalysisStore,
    InMemoryCacheStore, InMemoryCheckpointStore, InMemoryGuidanceStore,
};
use sa::{AgentStateSnapshot, AnalysisGraph, StructuredReport};
use sa::{
    AnalysisResult, GuidanceRule, PersistedTask, SingleAnalysisRequest, StoredCheckpoint,
    TaskStatus,
};

fn make_task(id: &str, symbol: &str, status: TaskStatus) -> PersistedTask {
    PersistedTask {
        task_id: id.to_string(),
        owner_username: "user1".to_string(),
        symbol: symbol.to_string(),
        stock_name: "Test".to_string(),
        market_type: "US".to_string(),
        analysis_date: "2026-01-01".to_string(),
        research_depth: "deep".to_string(),
        request: SingleAnalysisRequest {
            symbol: Some(symbol.to_string()),
            stock_code: None,
            stock_name: None,
            parameters: None,
            force_refresh: false,
        },
        status,
        progress: 0,
        current_step_name: String::new(),
        current_step_description: String::new(),
        message: String::new(),
        error_message: None,
        llm_token_usage: Default::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn analysis_store_insert_and_get() {
    let store = InMemoryAnalysisStore::new();
    let task = make_task("t1", "AAPL", TaskStatus::Pending);
    store.insert_task(&task).await.unwrap();
    let got = store.get_task("t1").await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().task_id, "t1");
}

#[tokio::test]
async fn analysis_store_get_missing() {
    let store = InMemoryAnalysisStore::new();
    let got = store.get_task("nonexistent").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn analysis_store_update_task() {
    let store = InMemoryAnalysisStore::new();
    let mut task = make_task("t1", "AAPL", TaskStatus::Pending);
    store.insert_task(&task).await.unwrap();
    task.status = TaskStatus::Running;
    task.progress = 50;
    store.update_task(&task).await.unwrap();
    let got = store.get_task("t1").await.unwrap().unwrap();
    assert_eq!(got.status, TaskStatus::Running);
    assert_eq!(got.progress, 50);
}

#[tokio::test]
async fn analysis_store_list_tasks() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Pending))
        .await
        .unwrap();
    store
        .insert_task(&make_task("t2", "GOOGL", TaskStatus::Running))
        .await
        .unwrap();
    let tasks = store.list_tasks(10, 0).await.unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn analysis_store_list_tasks_with_limit() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Pending))
        .await
        .unwrap();
    store
        .insert_task(&make_task("t2", "GOOGL", TaskStatus::Running))
        .await
        .unwrap();
    let tasks = store.list_tasks(1, 0).await.unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn analysis_store_list_tasks_for_user() {
    let store = InMemoryAnalysisStore::new();
    let mut task1 = make_task("t1", "AAPL", TaskStatus::Pending);
    task1.owner_username = "alice".to_string();
    let mut task2 = make_task("t2", "GOOGL", TaskStatus::Pending);
    task2.owner_username = "bob".to_string();
    store.insert_task(&task1).await.unwrap();
    store.insert_task(&task2).await.unwrap();
    let tasks = store.list_tasks_for_user("alice", 10, 0).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "t1");
}

#[tokio::test]
async fn analysis_store_find_cached_task() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Completed))
        .await
        .unwrap();
    store
        .insert_task(&make_task("t2", "AAPL", TaskStatus::Running))
        .await
        .unwrap();
    let found = store.find_cached_task("AAPL", "2026-01-01").await.unwrap();
    assert_eq!(found, Some("t1".to_string()));
}

#[tokio::test]
async fn analysis_store_find_cached_task_not_found() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Running))
        .await
        .unwrap();
    let found = store.find_cached_task("AAPL", "2026-01-01").await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn analysis_store_save_and_load_result() {
    let store = InMemoryAnalysisStore::new();
    let result = AnalysisResult {
        task_id: "t1".to_string(),
        report_id: "r1".to_string(),
        symbol: "AAPL".to_string(),
        stock_name: "Apple".to_string(),
        analysis_date: "2026-01-01".to_string(),
        market_type: "US".to_string(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: Default::default(),
        report: StructuredReport::default(),
        ic_report: StructuredReport::default(),
        created_at: "2026-01-01".to_string(),
    };
    store.save_result("t1", &result).await.unwrap();
    let loaded = store.load_result("t1").await.unwrap();
    assert!(loaded.is_some());
}

#[tokio::test]
async fn analysis_store_load_result_missing() {
    let store = InMemoryAnalysisStore::new();
    let loaded = store.load_result("nonexistent").await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn analysis_store_list_analyses() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Completed))
        .await
        .unwrap();
    store
        .insert_task(&make_task("t2", "GOOGL", TaskStatus::Completed))
        .await
        .unwrap();
    let analyses = store.list_analyses(None, 10).await.unwrap();
    assert_eq!(analyses.len(), 2);
}

#[tokio::test]
async fn analysis_store_list_analyses_filtered() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Completed))
        .await
        .unwrap();
    store
        .insert_task(&make_task("t2", "GOOGL", TaskStatus::Completed))
        .await
        .unwrap();
    let analyses = store.list_analyses(Some("AAPL"), 10).await.unwrap();
    assert_eq!(analyses.len(), 1);
}

#[tokio::test]
async fn analysis_store_delete_analysis() {
    let store = InMemoryAnalysisStore::new();
    store
        .insert_task(&make_task("t1", "AAPL", TaskStatus::Pending))
        .await
        .unwrap();
    store.delete_analysis("t1").await.unwrap();
    let got = store.get_task("t1").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn analysis_store_save_and_load_request() {
    let store = InMemoryAnalysisStore::new();
    let request = SingleAnalysisRequest {
        symbol: Some("AAPL".to_string()),
        stock_code: None,
        stock_name: None,
        parameters: None,
        force_refresh: false,
    };
    store.save_request("t1", &request).await.unwrap();
    let loaded = store.load_request("t1").await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().symbol, Some("AAPL".to_string()));
}

#[tokio::test]
async fn cache_store_set_and_get() {
    let store = InMemoryCacheStore::new();
    store.set("key1", b"value1", None).await.unwrap();
    let got = store.get("key1").await.unwrap();
    assert_eq!(got, Some(b"value1".to_vec()));
}

#[tokio::test]
async fn cache_store_get_missing() {
    let store = InMemoryCacheStore::new();
    let got = store.get("nonexistent").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn cache_store_delete() {
    let store = InMemoryCacheStore::new();
    store.set("key1", b"value1", None).await.unwrap();
    store.delete("key1").await.unwrap();
    let got = store.get("key1").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn cache_store_exists() {
    let store = InMemoryCacheStore::new();
    assert!(!store.exists("key1").await.unwrap());
    store.set("key1", b"value1", None).await.unwrap();
    assert!(store.exists("key1").await.unwrap());
}

#[tokio::test]
async fn cache_store_list_entries() {
    let store = InMemoryCacheStore::new();
    store.set("prefix:a", b"1", None).await.unwrap();
    store.set("prefix:b", b"2", None).await.unwrap();
    store.set("other:c", b"3", None).await.unwrap();
    let entries = store.list_entries("prefix:").await.unwrap();
    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn checkpoint_store_save_and_load() {
    let store = InMemoryCheckpointStore::new();
    let cp = StoredCheckpoint {
        task_id: "t1".to_string(),
        step_name: "step1".to_string(),
        stage: "init".to_string(),
        node: "node1".to_string(),
        step: 1,
        data: serde_json::json!({"key": "value"}),
        created_at: "2026-01-01".to_string(),
    };
    store.save_checkpoint("t1", "step1", &cp).await.unwrap();
    let loaded = store.load_checkpoint("t1").await.unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().step_name, "step1");
}

#[tokio::test]
async fn checkpoint_store_load_missing() {
    let store = InMemoryCheckpointStore::new();
    let loaded = store.load_checkpoint("nonexistent").await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn checkpoint_store_list_checkpoints() {
    let store = InMemoryCheckpointStore::new();
    let cp1 = StoredCheckpoint {
        task_id: "t1".to_string(),
        step_name: "step1".to_string(),
        stage: String::new(),
        node: String::new(),
        step: 1,
        data: serde_json::json!({}),
        created_at: "2026-01-01".to_string(),
    };
    let cp2 = StoredCheckpoint {
        task_id: "t1".to_string(),
        step_name: "step2".to_string(),
        stage: String::new(),
        node: String::new(),
        step: 2,
        data: serde_json::json!({}),
        created_at: "2026-01-02".to_string(),
    };
    store.save_checkpoint("t1", "step1", &cp1).await.unwrap();
    store.save_checkpoint("t1", "step2", &cp2).await.unwrap();
    let list = store.list_checkpoints("t1").await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn checkpoint_store_delete_checkpoints() {
    let store = InMemoryCheckpointStore::new();
    let cp = StoredCheckpoint {
        task_id: "t1".to_string(),
        step_name: "step1".to_string(),
        stage: String::new(),
        node: String::new(),
        step: 1,
        data: serde_json::json!({}),
        created_at: "2026-01-01".to_string(),
    };
    store.save_checkpoint("t1", "step1", &cp).await.unwrap();
    store.delete_checkpoints("t1").await.unwrap();
    let loaded = store.load_checkpoint("t1").await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn guidance_store_upsert_and_get() {
    let store = InMemoryGuidanceStore::new();
    let rule = GuidanceRule {
        id: "r1".to_string(),
        market_type: "US".to_string(),
        rule_type: "risk".to_string(),
        content: "Be careful".to_string(),
        priority: 1,
        enabled: true,
    };
    store.upsert_rule(&rule).await.unwrap();
    let got = store.get_rule("r1").await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().content, "Be careful");
}

#[tokio::test]
async fn guidance_store_get_missing() {
    let store = InMemoryGuidanceStore::new();
    let got = store.get_rule("nonexistent").await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn guidance_store_list_rules() {
    let store = InMemoryGuidanceStore::new();
    store
        .upsert_rule(&GuidanceRule {
            id: "r1".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 1".to_string(),
            priority: 1,
            enabled: true,
        })
        .await
        .unwrap();
    store
        .upsert_rule(&GuidanceRule {
            id: "r2".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 2".to_string(),
            priority: 2,
            enabled: true,
        })
        .await
        .unwrap();
    store
        .upsert_rule(&GuidanceRule {
            id: "r3".to_string(),
            market_type: "CN".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 3".to_string(),
            priority: 1,
            enabled: true,
        })
        .await
        .unwrap();
    let rules = store.list_rules("US").await.unwrap();
    assert_eq!(rules.len(), 2);
}

#[tokio::test]
async fn guidance_store_delete_rule() {
    let store = InMemoryGuidanceStore::new();
    store
        .upsert_rule(&GuidanceRule {
            id: "r1".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 1".to_string(),
            priority: 1,
            enabled: true,
        })
        .await
        .unwrap();
    store.delete_rule("r1").await.unwrap();
    let got = store.get_rule("r1").await.unwrap();
    assert!(got.is_none());
}
