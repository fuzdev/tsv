<script>
	// a same-line trailer + an own-line pre-`;` comment + a `;`→`else` comment reach one flush
	if (a) expr; // c1
	// c2
	// c3
	else fn1();
</script>
