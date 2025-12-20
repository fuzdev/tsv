<script>
	const newline = 'line\nbreak';
	const tab = 'tab\there';
	const quotes = '{"key": "value"}';
	const unicode = '\u0041\u0042\u0043';
	const hex = '\x41\x42\x43';
	const mixed = 'Hello\nWorld\t!';
</script>

<div>
	TypeScript escape tests:
	<span>{newline}</span>
	<span>{tab}</span>
	<span>{quotes}</span>
	<span>{unicode}</span>
	<span>{hex}</span>
	<span>{mixed}</span>
</div>
