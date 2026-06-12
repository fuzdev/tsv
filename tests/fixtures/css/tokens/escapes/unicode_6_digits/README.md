# Unicode Escapes - 6 Digits (Spec Maximum)

Tests unicode escapes up to CSS spec maximum (6 hex digits).

## Examples

```css
content: '\41';       /* 2 digits → 'A' */
content: '\20AC';     /* 4 digits → '€' */
content: '\1F4A9';    /* 5 digits → '💩' */
content: '\0000FF';   /* 6 digits → ÿ */
content: '\10FFFF';   /* 6 digits → max valid codepoint */
```

**CSS Spec**: 1-6 hex digits allowed. More than 6 is invalid (see `unicode_7_digits`).
