<script>
	switch  (  a  )  {
			// A glued run keeps its own line; the block drops below it.
			case   1  :
					/* block1 */    /* block2 */
					{   b()  ;   }
			// Blocks on their own lines keep them.
			case   2  :
					/* block1 */
					/* block2 */
					{   b()  ;   }
			// A line comment on the label keeps the glued run's line too.
			case   3  :     // line
					/* block1 */    /* block2 */
					{   b()  ;   }
			// A leading line comment keeps its line, and the run keeps its glue.
			case   4  :
					// line
					/* block1 */    /* block2 */
					{   b()  ;   }
			// Blank lines in the run are preserved, before the block as well as within it.
			case   5  :
					/* block1 */



					/* block2 */


					{   b()  ;   }
			// A block glued to the block statement leads it inline.
			case   6  :
					/* block */    {   b()  ;   }
			// A block on the label's line keeps the hug.
			case   7  :   /* block */    {   b()  ;   }
			// A block glued past a multiline block's closing line trails the label with it,
			// so nothing claims a line and the hug survives.
			case   8  :   /* block1
		block2 */    /* block3 */    {   b()  ;   }
			// A `default` label reads the same as a `case` label.
			default  :
					/* block1 */    /* block2 */
					{   b()  ;   }
	}
</script>
