<style>
	/* a connector binds its operand to the RIGHT, so one with nothing on its LEFT still
	belongs to the part that follows it: it is kept, in its source case */
	@supports and(a:b);
	@supports or(a:b);
	@supports AND(a:b);
	@supports and(a:b)and(c:d);

	/* every word of a run is kept, at either end of a part and in source order */
	@supports and and(a:b);
	@supports and or(a:b);
	@supports (a:b)and and(c:d);

	/* a comment keeps the side of the run the author wrote it on, inside the run too */
	@supports/* c */and(a:b);
	@supports and/* c */(a:b);
	@supports/* a */and/* b */or/* c */(d:e);
	@supports (a:b)and/* c */or(d:e);

	/* control: `not` is a segment of the part it prefixes, not a connector */
	@supports not(a:b);
	@supports and not(a:b);

	@container name1 and (a: b) {.class{color:red;}}

	/* the container name is optional, and a leading connector is not one */
	@container and (a: b) {.class{color:red;}}

	@container name1 and (a: b) and (c: d) {.class{color:red;}}
</style>
