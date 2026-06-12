<script>
	// Arrow hugging too long, standard expansion fits
	const a = b
		.method()
		.other((response) =>
			transform(response, option1, option2, option3, option4, option5_abcdefghijklmnopqrs),
		);

	// Contrast: fits with arrow hugging (shorter)
	const b2 = b
		.method()
		.other((response) =>
			transform(response, option1, option2, option3, option4, option5_abcdefghijklm),
		);

	// Neither fits: long base forces first-expanded or all-expanded
	const c = very_long_base_name_abcdefghijk
		.method_name_abcdef()
		.other((response) =>
			transform(response, option1, option2, option3, option4, option5_abcdefghijklmnopqrs),
		);
</script>
