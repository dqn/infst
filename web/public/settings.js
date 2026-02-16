// Copy token
document.getElementById('copy-token').addEventListener('click', function() {
  var input = document.getElementById('api-token');
  navigator.clipboard.writeText(input.value).then(function() {
    var btn = document.getElementById('copy-token');
    btn.textContent = 'Copied!';
    setTimeout(function() { btn.textContent = 'Copy'; }, 2000);
  });
});

// Regenerate token
document.getElementById('regen-token').addEventListener('click', function() {
  if (!confirm('Are you sure? Existing API clients will need the new token.')) return;
  fetch('/api/users/me/token/regenerate', { method: 'POST' })
    .then(function(res) { return res.json(); })
    .then(function(data) {
      if (data.apiToken) {
        document.getElementById('api-token').value = data.apiToken;
      }
    })
    .catch(function(err) { console.error('Failed to regenerate token:', err); });
});

// Toggle visibility
document.getElementById('is-public').addEventListener('change', function() {
  fetch('/api/users/me', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ isPublic: this.checked })
  })
  .catch(function(err) { console.error('Failed to update visibility:', err); });
});
