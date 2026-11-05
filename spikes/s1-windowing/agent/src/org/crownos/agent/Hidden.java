package org.crownos.agent;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

final class Hidden {
    static final Class<?>[] NO_TYPES = new Class<?>[0];

    private Hidden() {
    }

    static Class<?> type(String className) {
        try {
            return Class.forName(className);
        } catch (ClassNotFoundException e) {
            throw new HiddenApiException("class not found: " + className, e);
        }
    }

    static Object call(Object receiver, String methodName, Class<?>[] parameterTypes, Object... arguments) {
        return invoke(receiver.getClass(), receiver, methodName, parameterTypes, arguments);
    }

    static Object callStatic(Class<?> owner, String methodName, Class<?>[] parameterTypes, Object... arguments) {
        return invoke(owner, null, methodName, parameterTypes, arguments);
    }

    static Object construct(String className) {
        try {
            Constructor<?> constructor = type(className).getDeclaredConstructor();
            constructor.setAccessible(true);
            return constructor.newInstance();
        } catch (ReflectiveOperationException e) {
            throw new HiddenApiException("cannot construct " + className, unwrap(e));
        }
    }

    static Object field(Object receiver, String fieldName) {
        Field field = findField(receiver.getClass(), fieldName);
        try {
            field.setAccessible(true);
            return field.get(receiver);
        } catch (IllegalAccessException e) {
            throw new HiddenApiException("cannot read field " + fieldName, e);
        }
    }

    static int intField(Object receiver, String fieldName) {
        return (Integer) field(receiver, fieldName);
    }

    static boolean booleanField(Object receiver, String fieldName) {
        return (Boolean) field(receiver, fieldName);
    }

    private static Object invoke(Class<?> owner, Object receiver, String methodName, Class<?>[] parameterTypes,
            Object[] arguments) {
        try {
            Method method = owner.getMethod(methodName, parameterTypes);
            method.setAccessible(true);
            return method.invoke(receiver, arguments);
        } catch (ReflectiveOperationException e) {
            throw new HiddenApiException(owner.getName() + "." + methodName + " failed", unwrap(e));
        }
    }

    private static Field findField(Class<?> type, String fieldName) {
        for (Class<?> current = type; current != null; current = current.getSuperclass()) {
            for (Field candidate : current.getDeclaredFields()) {
                if (candidate.getName().equals(fieldName)) {
                    return candidate;
                }
            }
        }
        throw new HiddenApiException("field not found: " + type.getName() + "." + fieldName, null);
    }

    private static Throwable unwrap(ReflectiveOperationException e) {
        return e instanceof InvocationTargetException ? e.getCause() : e;
    }
}
