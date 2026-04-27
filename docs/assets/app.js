document.addEventListener('DOMContentLoaded', function () {
  var card = document.getElementById('go-sdk-card');
  if (!card) return;

  var toast = card.querySelector('.card-toast');

  card.addEventListener('click', function (e) {
    e.preventDefault();
    navigator.clipboard.writeText('go get go.openpit.dev/openpit').then(function () {
      if (!toast) return;
      toast.classList.add('visible');
      setTimeout(function () {
        toast.classList.remove('visible');
      }, 2000);
    });
  });
});
