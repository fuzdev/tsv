<script>
	// Call and new expressions format consistently (PR #18206)
	TelemetryTrustedValue(
		instance.capabilities.get(TerminalCapability?.PromptTypeDetection)?.promptType,
	);

	new TelemetryTrustedValue(
		instance.capabilities.get(TerminalCapability?.PromptTypeDetection)?.promptType,
	);
</script>
