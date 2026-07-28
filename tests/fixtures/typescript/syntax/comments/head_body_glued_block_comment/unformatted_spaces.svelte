<script>
	// If - own-line block comment glued to a block body
	if  (  a  )
			/* b */   {  fn();  }

	// While - own-line block comment glued to a block body
	while  (  a  )
   /* b */  {  fn();  }

	// For-of - own-line block comment glued to a block body
	for  (  x  of  y  )
		/* b */    {  fn();  }

	// Do - own-line block comment glued to a block body
	do
			/* b */  {  fn();  }  while  (  a  )  ;

	// Else - own-line block comment glued to a block body
	if  (  a  )  {  }  else
   /* b */   {  fn();  }

	// If - two glued block comments share the line they were authored on
	if  (  a  )
			/* b1 */   /* b2 */  {  fn();  }

	// If - own-line block comment glued to a keyword-opening non-block body
	if  (  a  )
   /* b */  for  (  ;  ;  )  {  break;  }

	// Else - own-line block comment glued to a keyword-opening non-block body
	if  (  a  )  {  }  else
			/* b */   for  (  ;  ;  )  {  break;  }

	// If - same-line block comment stays trailing (control)
	if  (  a  )   /* b */   {  fn();  }
</script>
