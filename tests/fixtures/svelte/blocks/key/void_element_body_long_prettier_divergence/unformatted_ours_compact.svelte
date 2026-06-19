<div>
	<!-- void element body @100 (head + body == 100): fits fully inline -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}<Spinner />{/key}

	<!-- void element body @101 (head + body > 100, head alone <= 100): construct overflows but the head fits alone → body drops to its own line (one-pass middle zone) -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}<Spinner />{/key}

	<!-- void element body, head alone > 100: head wraps AND body drops -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}<Spinner />{/key}

	<!-- an attributed element body drops the same way — no breakable-hug special case (uniform body-expand); prettier instead hugs the } and breaks the element internally -->
	{#key fn00000(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)}<Spinner class="loading-spinner-class" />{/key}
</div>
