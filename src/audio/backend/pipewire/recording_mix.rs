use super::{
    graph::Graph,
    route::{Links, Sink},
};
use crate::audio::Target;
use pipewire as pw;

pub(super) struct RecordingMix {
    inputs: Vec<Links>,
    pub sink: Sink,
    output: Target,
    microphone: Target,
    microphone_monitored: Option<bool>,
}

impl RecordingMix {
    pub fn new(core: &pw::core::CoreRc, graph: &Graph) -> Result<Option<Self>, String> {
        let Some((output, microphone)) = graph.recording_devices() else {
            return Ok(None);
        };
        let sink = Sink::new(
            core,
            2,
            format!("raspiboard.recording.{}", uuid::Uuid::now_v7()),
            "Raspiboard recording mix",
        )?;
        Ok(Some(Self {
            inputs: Vec::new(),
            sink,
            output,
            microphone,
            microphone_monitored: None,
        }))
    }

    pub fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        if let Some(error) = self.sink.error.borrow_mut().take() {
            return Err(error);
        }
        let output = graph.resolve(&self.output)?;
        let microphone = graph.resolve(&self.microphone)?;
        if graph.recording_devices().as_ref()
            != Some(&(self.output.clone(), self.microphone.clone()))
        {
            return Err("The default output or microphone changed. Start recording again.".into());
        }
        let Some(sink) = self.sink.id.get() else {
            return Ok(false);
        };
        let monitored = graph.feeds(microphone, output);
        check_monitoring(self.microphone_monitored, monitored)?;
        if self.inputs.is_empty() {
            self.microphone_monitored = Some(monitored);
            self.inputs.push(Links::mixing(output, sink));
            if !monitored {
                self.inputs.push(Links::mixing(microphone, sink));
            }
        }
        self.inputs.iter_mut().try_fold(true, |ready, links| {
            Ok(links.prepare_capture(core, graph)? && ready)
        })
    }
}

fn check_monitoring(previous: Option<bool>, monitored: bool) -> Result<(), String> {
    if previous.is_some_and(|previous| previous != monitored) {
        return Err(
            "Microphone monitoring changed. Start recording again to use the new audio mix.".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::graph::Kind;
    use super::*;

    #[test]
    fn microphone_monitoring_changes_cannot_silently_remove_or_double_voice() {
        let mut graph = Graph::default();
        assert!(check_monitoring(None, graph.feeds(20, 10)).is_ok());
        graph.insert(
            90,
            Kind::Link,
            [
                ("link.output.node".into(), "20".into()),
                ("link.input.node".into(), "10".into()),
            ]
            .into(),
        );
        assert!(check_monitoring(Some(true), graph.feeds(20, 10)).is_ok());
        assert!(
            check_monitoring(Some(false), graph.feeds(20, 10))
                .unwrap_err()
                .contains("monitoring changed")
        );
        graph.remove(90);
        assert!(check_monitoring(Some(true), graph.feeds(20, 10)).is_err());
        assert!(check_monitoring(Some(false), graph.feeds(20, 10)).is_ok());
    }
}
