# QMD Guide [[qmd_guide:__Namespace]]

Practical guide for QMD format.

- version: 1.0

## Basic Syntax [[basic_syntax:Section]]

### Creating Object [[ex_basic_object:Example]]

Basic example of object creation.

#### Code [[code:text]]

```markdown
## User [[alice]]

- name: Alice
- age: 30
```

**Result:** object with `__id: "alice"`.

### Data Types [[ex_data_types:Example]]

All supported types.

#### Code [[code:text]]

```markdown
## Config [[config]]

- text: Hello World
- number: 42
```

## References [[references:Section]]

### Basic Refs [[ex_basic_refs:Example]]

Simple references.

#### Code [[code:text]]

```markdown
## Order [[order1]]

- customer: [[#alice]]
```
