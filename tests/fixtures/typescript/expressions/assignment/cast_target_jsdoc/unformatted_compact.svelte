<script>
	/** @type {T} */(a)=1;
	/** @type {T} */(a.b)+=2;
	/** @type {T} */(a[i])*=3;

	// cast as a no-declaration for-of / for-in head target
	for (/** @type {T} */ (a) of items) {
	}
	for (/** @type {T} */ (a.b) in obj) {
	}
</script>
