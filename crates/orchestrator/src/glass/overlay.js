// Ghostlight Glass - Shared Web Components for Portable UI
class GhostlightOverlay extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
  }

  connectedCallback() {
    // Basic structure for the visual overlay (borders, etc.)
    this.shadowRoot.innerHTML = `
      <style>
        :host {
          pointer-events: none;
          position: fixed;
          top: 0;
          left: 0;
          width: 100vw;
          height: 100vh;
          z-index: 2147483647;
          display: block;
        }
        #gl-border {
          position: absolute;
          inset: 0;
          border: 4px solid var(--gl-sky, #38bdf8);
          opacity: 0;
          transition: opacity 0.3s ease;
          box-sizing: border-box;
        }
        :host([active]) #gl-border {
          opacity: 1;
        }
        #gl-narration {
          position: absolute;
          bottom: 20px;
          left: 50%;
          transform: translateX(-50%);
          background: rgba(12, 15, 20, 0.9);
          color: white;
          padding: 10px 20px;
          border-radius: 8px;
          font-family: system-ui, sans-serif;
          font-size: 14px;
          opacity: 0;
          transition: opacity 0.3s ease, transform 0.3s ease;
          box-shadow: 0 4px 12px rgba(0,0,0,0.5);
        }
        :host([narrating]) #gl-narration {
          opacity: 1;
          transform: translateX(-50%) translateY(-10px);
        }
      </style>
      <div id="gl-border"></div>
      <div id="gl-narration"></div>
    `;
    this.borderEl = this.shadowRoot.getElementById('gl-border');
    this.narrationEl = this.shadowRoot.getElementById('gl-narration');
  }

  static get observedAttributes() {
    return ['active', 'narrating', 'narration-text'];
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name === 'narration-text' && this.narrationEl) {
      this.narrationEl.textContent = newValue || '';
    }
  }

  presentNarration(text) {
    if (text) {
      this.setAttribute('narration-text', text);
      this.setAttribute('narrating', '');
    } else {
      this.removeAttribute('narrating');
      setTimeout(() => this.removeAttribute('narration-text'), 300);
    }
  }
}

customElements.define('ghostlight-overlay', GhostlightOverlay);

// Expose factory method to the window so the extension can inject it
window.installGhostlightOverlay = function() {
  if (!document.querySelector('ghostlight-overlay')) {
    const overlay = document.createElement('ghostlight-overlay');
    document.documentElement.appendChild(overlay);
    return overlay;
  }
  return document.querySelector('ghostlight-overlay');
};
