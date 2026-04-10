#ifndef ANNOTATIONRENDERER_H
#define ANNOTATIONRENDERER_H

#include <QPainter>
#include <QImage>
#include "Annotation.h"

namespace AnnotationRenderer {
void render(QPainter &p, const Annotation &a, const QImage &screenshotRegion = QImage());
}

#endif
