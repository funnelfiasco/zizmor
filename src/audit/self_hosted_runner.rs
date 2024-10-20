//! Audits workflows for usage of self-hosted runners,
//! which are frequently unsafe to use in public repositories
//! due to the potential for persistence between workflow runs.
//!
//! This audit is "pedantic" only, since zizmor can't detect
//! whether self-hosted runners are ephemeral or not.

use crate::{
    finding::{Confidence, Severity},
    AuditState,
};

use anyhow::Result;
use github_actions_models::{
    common::expr::ExplicitExpr,
    workflow::{job::RunsOn, Job},
};

use super::WorkflowAudit;

pub(crate) struct SelfHostedRunner {
    pub(crate) _state: AuditState,
}

impl WorkflowAudit for SelfHostedRunner {
    fn ident() -> &'static str
    where
        Self: Sized,
    {
        "self-hosted-runner"
    }

    fn desc() -> &'static str
    where
        Self: Sized,
    {
        "runs on a self-hosted runner"
    }

    fn new(state: AuditState) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(Self { _state: state })
    }

    fn audit<'w>(
        &self,
        workflow: &'w crate::models::Workflow,
    ) -> Result<Vec<crate::finding::Finding<'w>>> {
        let mut results = vec![];

        if !self._state.config.pedantic {
            log::info!("skipping self-hosted runner checks");
            return Ok(results);
        }

        for job in workflow.jobs() {
            let Job::NormalJob(normal) = *job else {
                continue;
            };

            match &normal.runs_on {
                RunsOn::Target(labels) => {
                    let Some(label) = labels.first() else {
                        continue;
                    };

                    if label == "self-hosted" {
                        // All self-hosted runners start with the 'self-hosted'
                        // label followed by any specifiers.
                        results.push(
                            Self::finding()
                                .confidence(Confidence::High)
                                .severity(Severity::Unknown)
                                .add_location(
                                    job.location()
                                        .with_keys(&["runs-on".into()])
                                        .annotated("self-hosted runner used here"),
                                )
                                .build(workflow)?,
                        );
                    } else if ExplicitExpr::from_curly(label).is_some() {
                        // The job might also have its runner expanded via an
                        // expression. Long-term we should perform this evaluation
                        // to increase our confidence, but for now we flag it as
                        // potentially expanding to self-hosted.
                        results.push(
                            Self::finding()
                                .confidence(Confidence::Low)
                                .severity(Severity::Unknown)
                                .add_location(
                                    job.location().with_keys(&["runs-on".into()]).annotated(
                                        "expression may expand into a self-hosted runner",
                                    ),
                                )
                                .build(workflow)?,
                        );
                    }
                }
                // NOTE: GHA docs are unclear on whether runner groups always
                // imply self-hosted runners or not. All examples suggest that they
                // do, but I'm not sure.
                // See: https://docs.github.com/en/actions/hosting-your-own-runners/managing-self-hosted-runners/managing-access-to-self-hosted-runners-using-groups
                // See: https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/choosing-the-runner-for-a-job
                RunsOn::Group {
                    group: _,
                    labels: _,
                } => results.push(
                    Self::finding()
                        .confidence(Confidence::Low)
                        .severity(Severity::Unknown)
                        .add_location(
                            job.location()
                                .with_keys(&["runs-on".into()])
                                .annotated("runner group implies self-hosted runner"),
                        )
                        .build(workflow)?,
                ),
            }
        }

        Ok(results)
    }
}
