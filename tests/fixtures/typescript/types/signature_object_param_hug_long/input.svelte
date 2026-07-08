<script lang="ts">
	// Type-member signatures (method, call, construct) hug a single object-type
	// parameter — `(o: {` stays on the signature line and the object type breaks —
	// the same rule value-param functions and declare/overload signatures use. A
	// short signature stays inline.
	interface I {
		short(o: { a: string }): boolean;
		methodName(o: {
			a: string;
			b: string;
			c: string;
			d: string;
			e: string;
			paddingg: string;
		}): boolean;
		(o: {
			a: string;
			b: string;
			c: string;
			d: string;
			e: string;
			paddingLongLongLong: string;
		}): boolean;
		new (o: {
			a: string;
			b: string;
			c: string;
			d: string;
			e: string;
			paddingLongLong: string;
		}): boolean;
	}
</script>
