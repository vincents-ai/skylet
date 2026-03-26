# Skylet Visual Identity & Branding Guide

## Logo

### Primary Logo
**File**: `logo.svg`  
**Format**: SVG (scalable)  
**Dimensions**: 100x100 (viewBox)  
**License**: Apache 2.0

### Design Concept

The Skylet logo features concentric circles with gradient opacity, representing:
- **Central Core**: The execution engine runtime
- **Expanding Rings**: Plugin capability extending outward
- **Red to Dark Red Gradient**: Energy and reliability
- **Dashed Circles**: Dynamic plugin loading and execution

### Color Palette

| Use | Color | Hex |
|-----|-------|-----|
| Primary | Bright Red | `#ef4444` |
| Accent | Dark Red | `#7f1d1d` |
| Gradient | Red → Dark Red | `#ef4444` → `#7f1d1d` |

### Logo Variations

#### Full Logo (Recommended)
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <!-- Concentric circles with gradient -->
</svg>
```

#### Icon Form
- Use for favicons, app icons
- Minimum size: 16x16 pixels
- Maintain aspect ratio

#### With Text
```
[Logo] Skylet
```
Use when space allows for branded header

### Usage Guidelines

#### ✅ DO
- Use on white or light backgrounds
- Scale proportionally
- Maintain sufficient whitespace
- Use in SVG format for web
- Optimize for web delivery

#### ❌ DON'T
- Change colors without approval
- Distort or rotate
- Add effects or shadows
- Rasterize for web (keep as SVG)
- Use on dark backgrounds (adjust opacity)

### Sizing

| Context | Size | Notes |
|---------|------|-------|
| README header | 200x200 | Primary display |
| Social media | 100x100 | Profile/icon |
| Favicon | 16x16, 32x32 | Browser tab |
| Documentation | 150x150 | Section headers |
| Badge | 24x24 | Inline with text |

## Typography

### Primary Font
**Recommended**: Inter, Roboto, or system sans-serif

### Project Names

| Context | Format | Example |
|---------|--------|---------|
| Project | Skylet | Skylet - Execution Engine |
| Release | Skylet v0.5.0 | Current beta release |
| ABI | Skylet ABI v2.0 | Plugin interface version |
| Crate | `skylet-*` | `skylet-abi`, `skylet-core` |

## Color Usage

### Primary Palette
```
#ef4444  Bright Red (Primary action, highlights)
#7f1d1d  Dark Red (Accents, borders)
```

### Extended Palette
```
#ffffff  White (Backgrounds)
#000000  Black (Text, borders)
#6b7280  Gray (Secondary text)
#10b981  Green (Success, passing)
#f59e0b  Amber (Warning, pending)
#ef4444  Red (Error, failure)
#3b82f6  Blue (Info, links)
```

## Brand Voice & Messaging

### Core Values
- **Secure** - Safety and reliability
- **Extensible** - Flexibility and power
- **Open** - Community and transparency
- **Fast** - Performance and efficiency

### Taglines
- "Secure plugin runtime for autonomous agents and microservices"
- "Extensible execution engine for microservices"
- "Open-source, battle-tested plugins"

### Key Messages
1. **ABI Stability**: "v2.0.0 frozen, no breaking changes until v3.0"
2. **Well Tested**: "1,079+ tests, zero warnings, v0.5.0 beta"
3. **Open Source**: "MIT OR Apache-2.0, standalone mode, zero external deps"
4. **Performance**: "Async/await, efficient job queue, hot reload"

## Logo Assets

### Location
- **Execution Engine**: `execution-engine/logo.svg`
- **Main Repo**: `skylet-logo.svg`

### Download
Available in:
- SVG (vector, recommended)
- PNG (raster, for compatibility)

### Permissions
All assets are:
- Licensed under Apache 2.0
- Free to use in documentation
- Free to use in community projects
- Attribution appreciated but not required

## Social Media

### Profile Image
Use logo on transparent background or white background

### Cover Image
Use gradient with Skylet name and tagline

### Hashtags
- `#Skylet`
- `#ExecutionEngine`
- `#PluginRuntime`
- `#OpenSource`
- `#Rust`

## Brand Guidelines by Platform

### GitHub
- Logo in README
- Logo in organization avatar (when public)
- Red theme in syntax highlighting
- Professional, technical tone

### crates.io
- Logo in crate description
- Red theme in documentation
- Link to GitHub repository
- Clear feature descriptions

### Documentation
- Logo at top of landing page
- Red accents for highlights
- Professional markdown formatting
- Clear hierarchy and structure

### Community
- Welcoming tone
- Professional graphics
- Consistent messaging
- Attribution of community work

## Visual Examples

### README Header
```markdown
# Skylet - Execution Engine

<div align="center">
  <img src="logo.svg" alt="Skylet Logo" width="200">
</div>

Secure, extensible plugin runtime for autonomous agents and microservices
```

### Badge Usage
```markdown
[![Skylet Badge](logo.svg)](https://github.com/vincents-ai/execution-engine)
```

### Social Media Post
```
🚀 Introducing Skylet v0.5.0!

The open-source execution engine is now available as a beta release.
- Stable plugin ABI (v2.0.0)
- 1,079+ tests passing
- Zero compiler warnings
- Apache 2.0 licensed

🔗 [GitHub](https://github.com/vincents-ai/execution-engine)
📚 [Docs](https://docs.rs/skylet-abi)
```

## Future Considerations

### Version 1.0.0
- May refine logo design
- Maintain current color scheme
- Consider additional variants
- Expand brand guidelines

### Long-term Vision
- Skylet becomes industry standard for plugin runtimes
- Consistent branding across ecosystem
- Clear visual hierarchy
- Recognition and trust in community

## Feedback & Updates

To suggest branding improvements:
1. Open GitHub issue
2. Include mockups or suggestions
3. Explain reasoning
4. Community discussion

Current Maintainers: Vincents AI Team

---

**Last Updated**: 2024-02-20  
**Version**: 1.0  
**License**: Apache 2.0
