use chrono::Utc;

use crate::llm::types::HasConfidence;
use crate::llm::{GeneratedDebateTurn, GeneratedRoleReport};
use crate::{AgentReportNode, AnalysisCheckpoint, AnalysisResult, DebateTurn};

pub(crate) fn push_analyst_node(result: &mut AnalysisResult, node: GeneratedRoleReport) {
    let key = node.key.clone();
    let summary = node.summary.clone();
    let title = node.title.clone();
    let (up_probability, down_probability, sideways_probability) =
        crate::llm::parse::normalize_probability_triplet(
            &node.up_probability,
            &node.down_probability,
            &node.sideways_probability,
        );
    let confidence = node.confidence_string();
    result.graph.analysts.push(AgentReportNode {
        key: node.key,
        title: node.title,
        agent: node.agent,
        summary: summary.clone(),
        detail: node.detail,
        evidence_points: node.evidence_points,
        up_probability,
        down_probability,
        sideways_probability,
        confidence,
        rationale: node.rationale,
        next_steps: node.next_steps,
        risks: node.risks,
    });
    push_checkpoint(result, &key, &title, "completed", summary);
}

pub(crate) fn debate_turn_from_generated(turn: &GeneratedDebateTurn) -> DebateTurn {
    DebateTurn {
        speaker: turn.speaker.clone(),
        stance: turn.stance.clone(),
        response: turn.response.clone(),
        confidence: turn.confidence_string(),
        evidence_points: turn.evidence_points.clone(),
        risks: turn.risks.clone(),
    }
}

pub(crate) fn push_checkpoint(
    result: &mut AnalysisResult,
    stage_key: &str,
    stage_name: &str,
    status: &str,
    summary: String,
) {
    result.graph.checkpoints.push(AnalysisCheckpoint {
        stage_key: stage_key.to_string(),
        stage_name: stage_name.to_string(),
        status: status.to_string(),
        summary,
        generated_at: Utc::now().to_rfc3339(),
    });
}
