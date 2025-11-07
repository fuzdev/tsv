<script>
	const zwsp = '\u200B';
	const zwnj = '\u200C';
	const zwj = '\u200D';
	const lrm = '\u200E';
	const rlm = '\u200F';
</script>

<p>zero-width space: &#8203;text</p>
<p>non-breaking space:&nbsp;text</p>
<p>zero-width joiner: &#x200D;text</p>
<p>left-to-right mark: &#8206;text</p>
<p>right-to-left mark: &#x200F;text</p>

<style>
	.class1 {
		content: '\200B';
	}

	.class2 {
		content: '\A0';
	}

	.class3 {
		content: '\200C';
	}

	.class4 {
		content: '\200D';
	}

	.class5 {
		content: '\200E';
	}

	.class6 {
		content: '\200F';
	}
</style>
