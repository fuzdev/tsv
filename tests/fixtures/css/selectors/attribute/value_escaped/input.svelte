<style>
	/* bare value whose escape encodes a quote stays bare (quoting would need re-escaping) */
	[data-attr1=x\'] {
		color: red;
	}

	[data-attr2=x\"] {
		color: blue;
	}

	/* bare value with a hex escape: quoted, escape kept verbatim */
	[data-attr3='x\27'] {
		color: green;
	}
</style>
