<script>
	const acute = 'e\u0301';
	const grave = 'a\u0300';
	const tilde = 'n\u0303';
	const cedilla = 'c\u0327';
</script>

<p>Combining acute: e&#769;</p>
<p>Combining grave: a&#x0300;</p>
<p>Combining tilde: n&#771;</p>
<p>Combining cedilla: c&#807;</p>

<style>
	.acute::before {
		content: 'e\301';
	}

	.grave::before {
		content: 'a\300';
	}

	.tilde::before {
		content: 'n\303';
	}

	.cedilla::before {
		content: 'c\327';
	}
</style>
