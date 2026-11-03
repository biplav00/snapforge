// AnnotationState::offsetAll + the REAL AnnotationCanvas::compositeImage().
//
// offsetAll: resizing the selection's top/left edge moves the canvas origin;
// every annotation (committed, in-progress, and every undo/redo snapshot) must
// shift by the opposite amount so it stays on the same screen pixels.
//
// compositeImage: tst_composite_dpr checks a copy of the painter logic; this
// drives the real canvas so a regression in AnnotationCanvas itself (double
// DPR scaling, wrong crop scale) is caught. Runs offscreen, linked against the
// SnapforgeClient fake.

#include <QtTest/QtTest>
#include <QApplication>
#include <QImage>
#include <QWidget>

#include "AnnotationCanvas.h"
#include "AnnotationState.h"

namespace {
Annotation rectAt(double x, double y, double w, double h) {
    Annotation a;
    a.id = AnnotationState::newId();
    a.tool = ToolType::Rect;
    a.color = Qt::black;
    a.strokeWidth = 2;
    a.data = RectData{x, y, w, h};
    return a;
}
} // namespace

class TestAnnotation : public QObject {
    Q_OBJECT
private slots:
    void offsetAll_translatesCommittedActiveAndHistory() {
        AnnotationState st;
        st.commitAnnotation(rectAt(10, 10, 5, 5));
        Annotation arrow;
        arrow.id = AnnotationState::newId();
        arrow.tool = ToolType::Arrow;
        arrow.data = ArrowData{0, 0, 20, 20};
        st.commitAnnotation(arrow);
        Annotation fh;
        fh.tool = ToolType::Freehand;
        fh.data = FreehandData{{QPointF(1, 1), QPointF(2, 2)}};
        st.setActiveAnnotation(fh);

        st.offsetAll(QPointF(-4, 3));

        const auto &r = std::get<RectData>(st.annotations().at(0).data);
        QCOMPARE(r.x, 6.0);
        QCOMPARE(r.y, 13.0);
        const auto &a = std::get<ArrowData>(st.annotations().at(1).data);
        QCOMPARE(a.endX, 16.0);
        QCOMPARE(a.endY, 23.0);
        const auto &f = std::get<FreehandData>(st.activeAnnotation()->data);
        QCOMPARE(f.points.at(1), QPointF(-2, 5));

        // Undo snapshot (one annotation) must be translated too.
        st.undo();
        QCOMPARE(st.annotations().size(), 1);
        QCOMPARE(std::get<RectData>(st.annotations().at(0).data).x, 6.0);
        st.redo();
        QCOMPARE(std::get<ArrowData>(st.annotations().at(1).data).startX, -4.0);
    }

    void offsetAll_zeroIsNoop() {
        AnnotationState st;
        st.commitAnnotation(rectAt(1, 1, 1, 1));
        const quint64 rev = st.committedRevision();
        st.offsetAll(QPointF(0, 0));
        QCOMPARE(st.committedRevision(), rev);
    }

    // Real compositeImage on a 2x capture: a mark at logical x=10 inside a
    // region starting at logical x=20 lands at physical x=20 in the output,
    // and the output is the region's pixel size.
    void compositeImage_realCanvas_scalesOnce() {
        QWidget overlay;
        overlay.resize(100, 100);
        QImage frame(200, 200, QImage::Format_ARGB32_Premultiplied); // 2x capture
        frame.fill(Qt::white);

        AnnotationState st;
        AnnotationCanvas canvas(&st, frame, &overlay);
        canvas.setRegion(20, 20, 50, 50);
        QCOMPARE(canvas.captureScale(), 2.0);

        Annotation mark;
        mark.id = AnnotationState::newId();
        mark.tool = ToolType::Highlight; // filled rect, easy to probe
        mark.color = Qt::black;
        mark.data = HighlightData{10, 10, 6, 6};
        st.commitAnnotation(mark);

        const QImage out = canvas.compositeImage();
        QCOMPARE(out.size(), QSize(100, 100));
        // Highlight paints at 30% opacity over white -> grey ~179; probe with
        // a looser threshold than "dark".
        int first = -1;
        for (int x = 0; x < out.width(); ++x)
            if (qGray(out.pixel(x, 26)) < 230) { first = x; break; }
        QCOMPARE(first, 20);
    }
};

QTEST_MAIN(TestAnnotation)
#include "tst_annotation.moc"
