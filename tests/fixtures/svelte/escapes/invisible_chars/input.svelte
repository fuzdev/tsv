<script>
	const zwsp = '\u200B';
	const zwnj = '\u200C';
	const zwj = '\u200D';
	const lrm = '\u200E';
	const rlm = '\u200F';
</script>

<p>Zero-width space: &#8203;word</p>
<p>Non-breaking space:&nbsp;word</p>
<p>Zero-width joiner: &#x200D;word</p>
<p>Left-to-right mark: &#8206;text</p>
<p>Right-to-left mark: &#x200F;text</p>

<style>
	.zwsp {
		content: '\200B';
	}

	.nbsp {
		content: '\A0';
	}

	.zwnj {
		content: '\200C';
	}

	.zwj {
		content: '\200D';
	}

	.lrm {
		content: '\200E';
	}

	.rlm {
		content: '\200F';
	}
</style>
