<script>
	const acute = 'e\u0301';
	const grave = 'a\u0300';
	const tilde = 'n\u0303';
	const cedilla = 'c\u0327';
</script>

<p>combining acute: e&#769;</p>
<p>combining grave: a&#x0300;</p>
<p>combining tilde: n&#771;</p>
<p>combining cedilla: c&#807;</p>

<style>
	.class1::before {
		content: 'e\301';
	}

	.class2::before {
		content: 'a\300';
	}

	.class3::before {
		content: 'n\303';
	}

	.class4::before {
		content: 'c\327';
	}
</style>
