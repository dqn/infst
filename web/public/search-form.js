document.getElementById('search-form').addEventListener('submit', function(e) {
  e.preventDefault();
  var username = this.querySelector('input').value.trim().toLowerCase();
  if (username) window.location.href = '/' + encodeURIComponent(username);
});
