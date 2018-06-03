FROM nginx:latest

COPY . /usr/share/nginx/html
RUN sed -i "s/app.js/app.js?`cat /usr/share/nginx/html/app.js | md5sum | awk '{print $1}'`/g" \
    /usr/share/nginx/html/index.html

